"""Closed public files and verified release links; build data, not runtime config."""

PAGE_FILES = (
    "download/index.html",
    "download/styles.css",
    "download/app.js",
    "download/canvas.js",
)
HOME_FILES = (
    "index.html",
    "home.css",
    "home.js",
    "cosmic.js",
)
SCREENSHOT_FILES = (
    "download/screenshots/workspace.png",
    "download/screenshots/office.png",
    "download/screenshots/review.png",
    "download/screenshots/lineage.png",
)
FONT_FILES = (
    "download/fonts/ubuntu-latin-400-normal.woff2",
    "download/fonts/ubuntu-latin-500-normal.woff2",
    "download/fonts/ubuntu-latin-700-normal.woff2",
    "download/fonts/Ubuntu-LICENSE.txt",
)
BRAND_FILES = (
    "tenant/aster/logo.svg",
    "tenant/aster/mark-light.png",
)
SITE_FILES = PAGE_FILES + SCREENSHOT_FILES + FONT_FILES + BRAND_FILES

REPOSITORY_URL = "https://github.com/aster-ai-lab/aster"
PAGES_URL = "https://aster-ai-lab.github.io/aster/"

# These are existing public stable artifacts, not a claim that a new desktop
# generation has been published. Updating this set requires checking the release.
DOWNLOAD_MANIFEST = (
    ("desktop", "macos", "0.1.1", "aarch64", REPOSITORY_URL + "/releases/download/v0.1.1/aster_0.1.1_aarch64.dmg"),
    ("desktop", "windows", "0.1.1", "x64", REPOSITORY_URL + "/releases/download/v0.1.1/aster_0.1.1_x64-setup.exe"),
    ("desktop", "kylin", "0.1.2", "x86_64", REPOSITORY_URL + "/releases/download/v0.1.2/aster_0.1.2_kylin-v10_x86_64.run"),
    ("cli", "macos", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-macos.tar.gz"),
    ("cli", "linux", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-linux.tar.gz"),
    ("cli", "windows", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-windows.zip"),
)
