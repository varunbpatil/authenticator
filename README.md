<a href="https://flathub.org/apps/details/com.belmoussaoui.Authenticator">
<img src="https://flathub.org/api/badge?svg&locale=en&light" />
</a>

> [!NOTE]
> This is a fork of [GNOME Authenticator](https://gitlab.gnome.org/World/Authenticator)
> that adds usability improvements, including:
>
> - Automatically selecting the first account in filtered search results
> - Navigating accounts and providers with `Up`/`Down`
> - Copying the selected account's code with `Ctrl+C`

# Authenticator

<img src="data/icons/com.belmoussaoui.Authenticator.svg" width="128px" height="128px" />

<p>Generate two-factor codes</p>

## Screenshots

<div align="center">
    <img src="./data/screenshots/screenshot1.png" />
</div>


## Features
 - Time-based/Counter-based/Steam methods support
 - SHA-1/SHA-256/SHA-512 algorithms support
 - QR code scanner using a camera or from a screenshot
 - Lock the application with a password
 - Beautiful UI
 - GNOME Shell search provider
 - Backup/Restore from/into known applications like FreeOTP+, Aegis (encrypted / plain-text), andOTP, Google Authenticator

## Getting in touch

If you have any questions regarding the use or development of Authenticator, please join us on our [#authenticator:gnome.org](https://matrix.to/#/#authenticator:gnome.org) channel.

## Hack on Authenticator

To build the development version of Authenticator and hack on the code see the
[general
guide](https://welcome.gnome.org/en/app/Authenticator/#getting-the-app-to-build)
for building GNOME apps with Flatpak and GNOME Builder.

## Credits

- We ship a database of providers based on [twofactorauth](https://github.com/2factorauth/twofactorauth), by the 2factorauth team
