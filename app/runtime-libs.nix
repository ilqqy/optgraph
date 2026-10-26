# Libraries GPUI loads on Linux (wgpu's Vulkan loader, Wayland, X11,
# xkbcommon, fontconfig): build inputs of the app and its runtime library path.
{
  vulkan-loader,
  wayland,
  libxkbcommon,
  libxcb,
  libx11,
  libxcursor,
  libxrandr,
  libxi,
  fontconfig,
  freetype,
  ...
}:
[
  vulkan-loader
  wayland
  libxkbcommon
  libxcb
  libx11
  libxcursor
  libxrandr
  libxi
  fontconfig
  freetype
]
