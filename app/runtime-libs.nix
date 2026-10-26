# Libraries GPUI loads on Linux (wgpu's Vulkan loader, Wayland, X11,
# xkbcommon, fontconfig): build inputs of the app and its runtime library path.
pkgs: [
  pkgs.vulkan-loader
  pkgs.wayland
  pkgs.libxkbcommon
  pkgs.libxcb
  pkgs.libx11
  pkgs.libxcursor
  pkgs.libxrandr
  pkgs.libxi
  pkgs.fontconfig
  pkgs.freetype
]
