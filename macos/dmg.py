# dmgbuild settings for Kiri.dmg, run from the repository root; scripts/release.sh passes `app` with -D.
app = defines["app"]  # noqa: F821
files = [app]
symlinks = {"Applications": "/Applications"}
format = "UDZO"
background = "macos/dmg-background.tiff"
window_rect = ((200, 120), (640, 400))
icon_size = 128
text_size = 13
show_status_bar = show_tab_view = show_toolbar = show_pathbar = show_sidebar = False
icon_locations = {"Kiri.app": (170, 166), "Applications": (470, 166)}
