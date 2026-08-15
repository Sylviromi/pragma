# Pragma — AUR package

This folder is the canonical source for Arch Linux (`AUR`) packaging of Pragma.

## What it does

A proper Arch package: builds from the **source tarball** of the tag-matching
release, then installs the binary to `/usr/bin`, an app-menu entry
(`pragma.desktop`) and hicolor icons — so Pragma shows up in the application
menu/grid like a first-class desktop app. This is unlike the portable `.AppImage`
(which only gets a menu entry if AppImageLauncher/`appimaged` is installed).

## Releasing / keeping it in sync

After an app release, run once (computes the new source tarball's sha256 and
rebuilds `.SRCINFO`):

```bash
./scripts/aur-update.sh 0.1.3
git add -A && git commit -m "chore(aur): bump to 0.1.3" && git push
```

## Submitting to the AUR (maintainer)

AUR packages live in their own git repo (`aur.archlinux.org/<name>.git`) —
GitHub can't publish there. To submit/update:

```bash
# clone the AUR repo (needs an AUR account + SSH key), then copy this over
git clone ssh://aur@aur.archlinux.org/pragma.git /tmp/aur-pragma
cp distrib/aur/PKGBUILD distrib/aur/.SRCINFO /tmp/aur-pragma/
cd /tmp/aur-pragma
git add PKGBUILD .SRCINFO
git commit -m "0.1.3"
git push
```

> Check the package name is free on the AUR first; rename `pkgname` if `pragma`
> is taken.

## Installing (users on Arch)

```bash
yay -S pragma          # or: paru -S pragma
pragma
```

The package name on this machine is `pragma`; if submitted under another name,
install that.
