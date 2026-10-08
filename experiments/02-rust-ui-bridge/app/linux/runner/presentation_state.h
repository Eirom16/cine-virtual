#pragma once
#include <algorithm>
#include <cstdint>

// Guarded by the presenter notification mutex; contains no playback decisions.
struct PresentationState {
  int width = 640, height = 360;
  uint64_t generation = 1, completed = 0;
  bool revealed = true;
  bool resize(int w, int h) {
    if (w <= 0 || h <= 0)
      return false;
    const double scale = std::min(1.0, 4096.0 / std::max(w, h));
    w = std::max(1, static_cast<int>(w * scale));
    h = std::max(1, static_cast<int>(h * scale));
    if (width == w && height == h)
      return false;
    width = w;
    height = h;
    return true;
  }
  void clear() {
    ++generation;
    revealed = false;
    completed = 0;
  }
  void reveal() {
    ++generation;
    revealed = true;
    completed = 0;
  }
  bool finish(uint64_t rendered) {
    if (rendered != generation || !revealed)
      return false;
    completed = rendered;
    return true;
  }
};
