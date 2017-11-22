---
layout: default
overview: true
---
Iris is a frontend for the Mopidy music server. The purpose of Iris is to pull together a variety of music sources and present them in an extremely functional and user-friendly experience. The primary music source (at this time) is Spotify, provided by the Mopidy-Spotify backend.

## Installing
### Requirements
- [Mopidy](https://docs.mopidy.com/en/latest/installation/)
- [Mopidy-Spotify](https://github.com/mopidy/mopidy-spotify)
- [Mopidy-Local-SQLite](https://github.com/mopidy/mopidy-local-sqlite) (recommended)

### Installation
#### To install using PIP
1. Run `sudo pip install Mopidy-Iris`

#### To install manually
1. Pull code base `git clone git@github.com:jaedb/Iris.git`
2. Install package `python setup.py install`

### Configuration
Iris will work for most users out-of-the-box. You can customise some of the configuration settings if you wish.

These **optional** values are defined in your `mopidy.conf` file (typically `~/.config/mopidy/mopidy.conf`). They can be set in the `[iris]` section.
- `country` Spotify-based country code (defaults to `nz`)
- `locale` Spotify-based locale code (defaults to `en_NZ`)
- `spotify_authorization_url` URL to use as Spotify authentication proxy (defaults to `https://jamesbarnsley.co.nz/auth_spotify.php`)
- `lastfm_authorization_url ` URL to use as LastFM authentication proxy (defaults to `https://jamesbarnsley.co.nz/auth_lastfm.php`)

## Upgrading
You can upgrade from Iris under _Settings_ > _Upgrade_. This is in beta, and you will need to restart Mopidy for the upgrade to complete. Alternatively run `sudo pip install --upgrade Mopidy-Iris`.

If you're experiencing dependency issues, try installing without dependencies `sudo pip install --upgrade --no-deps Mopidy-Iris`.
