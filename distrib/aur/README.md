# Pragma — AUR package

This folder is the canonical source for Arch Linux (`AUR`) packaging of Pragma.

## What it does

A proper Arch package: builds from the **source tarball** of the tag-matching
release, then installs the binary to `/usr/bin`, an app-menu entry
(`pragma.desktop`) and hicolor icons — so Pragma shows up in the application
menu/grid like a first-class desktop app. This is unlike the portable `.AppImage`
(which only gets a menu entry if AppImageLauncher/`appimaged` is installed).

## Releasing / keeping it in sync (one command)

After an app release, publish to the AUR with:

```bash
./scripts/aur-deploy.sh 0.1.3                 # bump + push
./scripts/aur-deploy.sh 0.1.3 --clone          # first time on a machine
```

That bumps the PKGBUILD (pkgver + source sha256), regenerates `.SRCINFO`, and
pushes to `aur.archlinux.org/pragma` in one go. It just needs your AUR SSH key
('`AUR_DIR`' defaults to `~/aur/pragma`; override with `AUR_DIR=/path`).

## Submitting the FIRST package (one-time)

1. Register the package name on the AUR website: **aur.archlinux.org → Submit**
   (this creates the empty git repo). If `pragma` is taken, rename `pkgname`.
2. Register your SSH key in **AUR account settings → SSH keys**.
3. Then clone + publish with `./scripts/aur-deploy.sh 0.1.2 --clone`.

Under the hood it's just: copy `distrib/aur/{PKGBUILD,.SRCINFO}` into the AUR
repo, commit `upgpkg: pragma <ver>`, push. You can also do that by hand if you
prefer (see below).

## Manual alternative (no script)

```bash
git clone ssh://aur@aur.archlinux.org/pragma.git /tmp/aur-pragma
cp distrib/aur/PKGBUILD distrib/aur/.SRCINFO /tmp/aur-pragma/
cd /tmp/aur-pragma
git add PKGBUILD .SRCINFO
git commit -m "upgpkg: pragma 0.1.3"
git push
```

## Installing (users on Arch)

```bash
yay -S pragma          # or: paru -S pragma
pragma
```

The package name on this machine is `pragma`; if submitted under another name,
install that.
