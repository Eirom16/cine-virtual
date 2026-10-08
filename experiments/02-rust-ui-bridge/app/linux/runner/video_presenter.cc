#include "video_presenter.h"
#include "presentation_state.h"
#include <algorithm>
#include <atomic>
#include <condition_variable>
#include <epoxy/egl.h>
#include <epoxy/gl.h>
#include <gmodule.h>
#include <mpv/client.h>
#include <mpv/render_gl.h>
#include <mutex>
#include <thread>

// All mpv_render calls belong to this worker and its private EGL context.
// The raster thread only copies completed GPU pixels into its consumer texture.
struct Presenter {
  mpv_handle *mpv = nullptr;
  bool spike = false;
  GModule *bridge = nullptr;
  uint64_t lease = 0;
  int32_t (*release)(uint64_t) = nullptr;
  std::atomic<bool> closing{false};
  FlTextureRegistrar *registrar = nullptr;
  FlTexture *texture = nullptr;
  EGLDisplay display = EGL_NO_DISPLAY;
  EGLContext context = EGL_NO_CONTEXT;
  EGLContext flutter_context = EGL_NO_CONTEXT;
  std::thread worker;
  std::mutex mutex;
  std::mutex gpu;
  std::condition_variable wake;
  bool stop = false, pending = true, ready = false;
  std::atomic<int> error{0};
  PresentationState state;
  std::atomic<bool> presentable{false};
  int rendered_width = 0, rendered_height = 0;
  GLuint producer = 0, fbo = 0, consumer = 0, copy_fbo = 0;
  uint64_t frames = 0;
  int consumer_width = 0, consumer_height = 0;
};
struct _CineTexture {
  FlTextureGL parent_instance;
  Presenter *presenter;
};
G_DECLARE_FINAL_TYPE(CineTexture, cine_texture, CINE, TEXTURE, FlTextureGL)
G_DEFINE_TYPE(CineTexture, cine_texture, fl_texture_gl_get_type())

static void updated(void *data) {
  auto *p = static_cast<Presenter *>(data);
  {
    std::lock_guard<std::mutex> lock(p->mutex);
    p->pending = true;
  }
  p->wake.notify_one();
}
static void *proc(void *, const char *name) {
  return reinterpret_cast<void *>(eglGetProcAddress(name));
}
static void render_worker(Presenter *p, mpv_handle *mpv) {
  if (!eglMakeCurrent(p->display, EGL_NO_SURFACE, EGL_NO_SURFACE, p->context)) {
    p->error = -1001;
    return;
  }
  mpv_render_context *render = nullptr;
  mpv_opengl_init_params gl{proc, nullptr};
  int advanced = 1;
  mpv_render_param init[] = {{MPV_RENDER_PARAM_API_TYPE,
                              const_cast<char *>(MPV_RENDER_API_TYPE_OPENGL)},
                             {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl},
                             {MPV_RENDER_PARAM_ADVANCED_CONTROL, &advanced},
                             {MPV_RENDER_PARAM_INVALID, nullptr}};
  int rc = mpv_render_context_create(&render, mpv, init);
  if (rc < 0) {
    p->error = rc;
    eglMakeCurrent(p->display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    return;
  }
  glGenTextures(1, &p->producer);
  glBindTexture(GL_TEXTURE_2D, p->producer);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
  glGenFramebuffers(1, &p->fbo);
  mpv_render_context_set_update_callback(render, updated, p);
  {
    std::lock_guard<std::mutex> lock(p->mutex);
    p->ready = true;
  }
  while (true) {
    std::unique_lock<std::mutex> lock(p->mutex);
    p->wake.wait(lock, [p] { return p->stop || p->pending; });
    if (p->stop)
      break;
    p->pending = false;
    const int w = p->state.width, h = p->state.height;
    const auto generation = p->state.generation;
    // update/render can synchronously invoke updated: never retain its mutex.
    lock.unlock();
    mpv_render_context_update(render);
    std::unique_lock<std::mutex> gpu_lock(p->gpu);
    lock.lock();
    if (w != p->rendered_width || h != p->rendered_height) {
      glBindTexture(GL_TEXTURE_2D, p->producer);
      glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA,
                   GL_UNSIGNED_BYTE, nullptr);
      glBindFramebuffer(GL_FRAMEBUFFER, p->fbo);
      glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
                             GL_TEXTURE_2D, p->producer, 0);
      p->rendered_width = w;
      p->rendered_height = h;
    }
    lock.unlock();
    glBindFramebuffer(GL_FRAMEBUFFER, p->fbo);
    if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
      p->error = -1003;
      break;
    }
    mpv_opengl_fbo target{static_cast<int>(p->fbo), w, h, GL_RGBA8};
    int flip = 0, block = 0;
    mpv_render_param params[] = {
        {MPV_RENDER_PARAM_OPENGL_FBO, &target},
        {MPV_RENDER_PARAM_FLIP_Y, &flip},
        {MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME, &block},
        {MPV_RENDER_PARAM_INVALID, nullptr}};
    // Producer is protected separately from callback notification (below).
    rc = mpv_render_context_render(render, params);
    glFinish();
    if (rc < 0)
      p->error = rc;
    lock.lock();
    ++p->frames;
    p->presentable = p->state.finish(generation);
    lock.unlock();
    gpu_lock.unlock();
    fl_texture_registrar_mark_texture_frame_available(p->registrar, p->texture);
  }
  mpv_render_context_set_update_callback(render, nullptr, nullptr);
  mpv_render_context_free(render);
  {
    std::lock_guard<std::mutex> gpu_lock(p->gpu);
    glDeleteFramebuffers(1, &p->fbo);
    glDeleteTextures(1, &p->producer);
    p->producer = 0;
    p->fbo = 0;
    p->rendered_width = 0;
    p->rendered_height = 0;
  }
  eglMakeCurrent(p->display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
}
static gboolean populate(FlTextureGL *texture, uint32_t *target, uint32_t *name,
                         uint32_t *width, uint32_t *height, GError **) {
  Presenter *p = CINE_TEXTURE(texture)->presenter;
  if (!p || p->closing)
    return FALSE;
  std::unique_lock<std::mutex> lifecycle(p->mutex);
  if (p->context == EGL_NO_CONTEXT) {
    p->display = eglGetCurrentDisplay();
    EGLContext shared = eglGetCurrentContext();
    EGLint config_id = 0;
    if (p->display == EGL_NO_DISPLAY || shared == EGL_NO_CONTEXT ||
        !eglQueryContext(p->display, shared, EGL_CONFIG_ID, &config_id)) {
      p->error = -1002;
      return FALSE;
    }
    EGLint attrs[] = {EGL_CONFIG_ID, config_id, EGL_NONE};
    EGLConfig config = nullptr;
    EGLint count = 0;
    if (!eglChooseConfig(p->display, attrs, &config, 1, &count) || count != 1) {
      p->error = -1002;
      return FALSE;
    }
    EGLint context_attrs[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
    p->context = eglCreateContext(p->display, config, shared, context_attrs);
    if (p->context == EGL_NO_CONTEXT) {
      p->error = -1002;
      return FALSE;
    }
    p->flutter_context = shared;
    if (p->mpv)
      p->worker = std::thread(render_worker, p, p->mpv);
  }
  if (eglGetCurrentContext() != p->flutter_context) {
    // A replacement engine context invalidates sharing. Report surface loss;
    // recovery requires restarting this engine rather than reading stale GL
    // IDs.
    p->error = -1005;
    return FALSE;
  }
  lifecycle.unlock();
  if (!p->consumer) {
    glGenTextures(1, &p->consumer);
    glBindTexture(GL_TEXTURE_2D, p->consumer);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 1, 1, 0, GL_RGBA, GL_UNSIGNED_BYTE,
                 nullptr);
  }
  std::lock_guard<std::mutex> gpu_lock(p->gpu);
  if (p->rendered_width > 0 && p->presentable && !p->error) {
    if (!p->copy_fbo)
      glGenFramebuffers(1, &p->copy_fbo);
    GLint old_fbo;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &old_fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, p->copy_fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                           p->producer, 0);
    glBindTexture(GL_TEXTURE_2D, p->consumer);
    if (p->consumer_width != p->rendered_width ||
        p->consumer_height != p->rendered_height) {
      glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, p->rendered_width,
                   p->rendered_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
      p->consumer_width = p->rendered_width;
      p->consumer_height = p->rendered_height;
    }
    if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
      p->error = -1003;
      glBindFramebuffer(GL_FRAMEBUFFER, old_fbo);
      return FALSE;
    }
    glCopyTexSubImage2D(GL_TEXTURE_2D, 0, 0, 0, 0, 0, p->rendered_width,
                        p->rendered_height);
    glFinish();
    glBindFramebuffer(GL_FRAMEBUFFER, old_fbo);
    if (glGetError() != GL_NO_ERROR)
      p->error = -1004;
  } else {
    glBindTexture(GL_TEXTURE_2D, p->consumer);
    const unsigned char black[] = {0, 0, 0, 255};
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 1, 1, 0, GL_RGBA, GL_UNSIGNED_BYTE,
                 black);
    p->consumer_width = p->consumer_height = 1;
  }
  *target = GL_TEXTURE_2D;
  *name = p->consumer;
  *width = p->consumer_width;
  *height = p->consumer_height;
  return TRUE;
}
static void finalize_texture(GObject *object);
static void cine_texture_class_init(CineTextureClass *klass) {
  G_OBJECT_CLASS(klass)->finalize = finalize_texture;
  FL_TEXTURE_GL_CLASS(klass)->populate = populate;
}
static void cine_texture_init(CineTexture *self) { self->presenter = nullptr; }

static Presenter *active = nullptr;
static FlTextureRegistrar *registrar = nullptr;
static void detach() {
  if (!active)
    return;
  auto *p = active;
  std::thread worker;
  mpv_handle *mpv;
  {
    std::lock_guard<std::mutex> lock(p->mutex);
    p->stop = true;
    mpv = p->mpv;
    p->mpv = nullptr;
    worker = std::move(p->worker);
  }
  p->wake.notify_one();
  if (worker.joinable())
    worker.join();
  if (p->spike && mpv)
    mpv_terminate_destroy(mpv);
  if (p->lease)
    p->release(p->lease);
  if (p->bridge)
    g_module_close(p->bridge);
  p->bridge = nullptr;
  p->lease = 0;
  p->mpv = nullptr;
  {
    std::lock_guard<std::mutex> lock(p->mutex);
    p->state.clear();
    p->presentable = false;
    p->ready = false;
    p->stop = false;
    p->pending = true;
    p->error = 0;
  }
  fl_texture_registrar_mark_texture_frame_available(p->registrar, p->texture);
}
static void shutdown_presenter() {
  if (!active)
    return;
  detach();
  active->closing = true;
  // Registrar retains the texture until engine shutdown; no mid-raster free.
  g_object_unref(active->texture);
  active = nullptr;
}
static void finalize_texture(GObject *object) {
  auto *p = CINE_TEXTURE(object)->presenter;
  if (p) {
    if (p->context != EGL_NO_CONTEXT) {
      eglMakeCurrent(p->display, EGL_NO_SURFACE, EGL_NO_SURFACE, p->context);
      glDeleteTextures(1, &p->consumer);
      eglMakeCurrent(p->display, EGL_NO_SURFACE, EGL_NO_SURFACE,
                     EGL_NO_CONTEXT);
      eglDestroyContext(p->display, p->context);
    }
    delete p;
  }
  G_OBJECT_CLASS(cine_texture_parent_class)->finalize(object);
}
static void ensure_presenter() {
  if (active)
    return;
  active = new Presenter();
  active->registrar = registrar;
  auto *t = CINE_TEXTURE(g_object_new(cine_texture_get_type(), nullptr));
  t->presenter = active;
  active->texture = FL_TEXTURE(t);
  fl_texture_registrar_register_texture(registrar, active->texture);
}
static void method(FlMethodChannel *, FlMethodCall *call, gpointer window) {
  const char *action = fl_method_call_get_name(call);
  FlValue *args = fl_method_call_get_args(call);
  if (g_str_equal(action, "fullscreen")) {
    if (fl_value_get_bool(args))
      gtk_window_fullscreen(GTK_WINDOW(window));
    else
      gtk_window_unfullscreen(GTK_WINDOW(window));
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  if (g_str_equal(action, "initialize")) {
    ensure_presenter();
    g_autoptr(FlValue) result =
        fl_value_new_int(fl_texture_get_id(active->texture));
    fl_method_call_respond_success(call, result, nullptr);
    return;
  }
  if (g_str_equal(action, "resize")) {
    if (active) {
      std::lock_guard<std::mutex> lock(active->mutex);
      active->state.resize(static_cast<int>(fl_value_get_int(
                               fl_value_lookup_string(args, "width"))),
                           static_cast<int>(fl_value_get_int(
                               fl_value_lookup_string(args, "height"))));
      active->pending = true;
      active->wake.notify_one();
    }
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  if (g_str_equal(action, "clear") || g_str_equal(action, "reveal")) {
    if (active) {
      {
        std::lock_guard<std::mutex> lock(active->mutex);
        if (g_str_equal(action, "clear"))
          active->state.clear();
        else
          active->state.reveal();
        active->presentable = false;
        active->pending = true;
      }
      active->wake.notify_one();
      fl_texture_registrar_mark_texture_frame_available(active->registrar,
                                                        active->texture);
    }
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  if (g_str_equal(action, "bind")) {
    ensure_presenter();
    detach();
    auto *p = active;
    std::lock_guard<std::mutex> lifecycle(p->mutex);
    if (p->context == EGL_NO_CONTEXT) {
      fl_method_call_respond_error(call, "VIDEO_CONTEXT_PENDING", nullptr,
                                   nullptr, nullptr);
      return;
    }
    const char *path =
        fl_value_get_string(fl_value_lookup_string(args, "library"));
    uint64_t handle = fl_value_get_int(fl_value_lookup_string(args, "handle"));
    p->bridge = g_module_open(path, G_MODULE_BIND_LAZY);
    uint64_t (*acquire)(uint64_t, void **) = nullptr;
    if (!p->bridge ||
        !g_module_symbol(p->bridge, "cine_bridge_video_acquire",
                         reinterpret_cast<gpointer *>(&acquire)) ||
        !g_module_symbol(p->bridge, "cine_bridge_video_release",
                         reinterpret_cast<gpointer *>(&p->release))) {
      fl_method_call_respond_error(call, "VIDEO_BRIDGE_FAILED", nullptr,
                                   nullptr, nullptr);
      return;
    }
    void *mpv = nullptr;
    p->lease = acquire(handle, &mpv);
    if (!p->lease) {
      fl_method_call_respond_error(call, "VIDEO_LEASE_FAILED", nullptr, nullptr,
                                   nullptr);
      return;
    }
    p->mpv = static_cast<mpv_handle *>(mpv);
    p->spike = false;
    p->worker = std::thread(render_worker, p, p->mpv);
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  if (g_str_equal(action, "spike")) {
    ensure_presenter();
    detach();
    auto *p = active;
    std::unique_lock<std::mutex> lifecycle(p->mutex);
    p->state.reveal();
    p->spike = true;
    p->mpv = mpv_create();
    mpv_set_option_string(p->mpv, "vo", "libmpv");
    mpv_set_option_string(p->mpv, "ao", "null");
    mpv_set_option_string(p->mpv, "pause", "yes");
    mpv_set_option_string(p->mpv, "terminal", "no");
    mpv_set_option_string(p->mpv, "hwdec", "no");
    if (mpv_initialize(p->mpv) < 0) {
      mpv_terminate_destroy(p->mpv);
      p->mpv = nullptr;
      fl_method_call_respond_error(call, "VIDEO_INIT_FAILED", nullptr, nullptr,
                                   nullptr);
      return;
    }
    if (p->context != EGL_NO_CONTEXT)
      p->worker = std::thread(render_worker, p, p->mpv);
    g_autoptr(FlValue) result = fl_value_new_int(fl_texture_get_id(p->texture));
    fl_method_call_respond_success(call, result, nullptr);
    return;
  }
  if (g_str_equal(action, "dispose")) {
    detach();
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  if (!active) {
    fl_method_call_respond_error(call, "VIDEO_NOT_ATTACHED", nullptr, nullptr,
                                 nullptr);
    return;
  }
  if (g_str_equal(action, "status")) {
    g_autoptr(FlValue) value = fl_value_new_map();
    std::unique_lock<std::mutex> lock(active->mutex);
    fl_value_set_string_take(
        value, "context", fl_value_new_bool(active->context != EGL_NO_CONTEXT));
    fl_value_set_string_take(value, "ready", fl_value_new_bool(active->ready));
    fl_value_set_string_take(value, "error", fl_value_new_int(active->error));
    fl_value_set_string_take(value, "frames", fl_value_new_int(active->frames));
    fl_value_set_string_take(value, "width",
                             fl_value_new_int(active->state.width));
    fl_value_set_string_take(value, "height",
                             fl_value_new_int(active->state.height));
    fl_value_set_string_take(
        value, "backend",
        fl_value_new_string(G_OBJECT_TYPE_NAME(gdk_display_get_default())));
    fl_value_set_string_take(value, "generation",
                             fl_value_new_int(active->state.generation));
    fl_value_set_string_take(value, "completed_generation",
                             fl_value_new_int(active->state.completed));
    lock.unlock();
    if (active->spike && active->mpv) {
      double position = 0;
      mpv_get_property(active->mpv, "time-pos", MPV_FORMAT_DOUBLE, &position);
      fl_value_set_string_take(value, "position_ms",
                               fl_value_new_int(position * 1000));
    }
    fl_method_call_respond_success(call, value, nullptr);
    return;
  }
  if (active->spike && g_str_equal(action, "command")) {
    const char *cmd = fl_value_get_string(fl_value_get_list_value(args, 0));
    const char *value = fl_value_get_string(fl_value_get_list_value(args, 1));
    const char *extra =
        fl_value_get_length(args) > 2
            ? fl_value_get_string(fl_value_get_list_value(args, 2))
            : nullptr;
    const char *command[] = {cmd, value, extra, nullptr};
    int rc = mpv_command(active->mpv, command);
    if (rc < 0) {
      fl_method_call_respond_error(call, "SPIKE_COMMAND_FAILED", nullptr,
                                   nullptr, nullptr);
      return;
    }
    fl_method_call_respond_success(call, nullptr, nullptr);
    return;
  }
  fl_method_call_respond_not_implemented(call, nullptr);
}
void cine_video_register(FlView *view, GtkWindow *window) {
  registrar = fl_engine_get_texture_registrar(fl_view_get_engine(view));
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel = fl_method_channel_new(
      fl_engine_get_binary_messenger(fl_view_get_engine(view)),
      "cine.desktop/video", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method, window, nullptr);
  g_signal_connect_swapped(window, "destroy", G_CALLBACK(shutdown_presenter),
                           nullptr);
}
