#include "presentation_state.h"
#include <cassert>
int main() {
  PresentationState s;
  assert(s.resize(1920, 1080));
  assert(!s.resize(1920, 1080));
  assert(!s.resize(0, 100));
  assert(s.resize(7680, 4320));
  assert(s.width == 4096 && s.height == 2304);
  const auto old = s.generation;
  s.clear();
  assert(!s.finish(old));
  assert(!s.finish(s.generation));
  s.reveal();
  assert(!s.finish(old));
  assert(s.finish(s.generation));
  s.clear();
  assert(s.completed == 0 && !s.revealed);
  s.reveal();
  assert(s.finish(s.generation));
}
