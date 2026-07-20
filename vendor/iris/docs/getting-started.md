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

### User Configuration

Each user which connects to Iris has their own settings, stored in `localStorage`. You can use the `Application` inspector in Developer Tools to view this data, or type `_store.getState()` in the console. To share settings between users, you can use `Share configuration` button in **Settings -> Advanced**.

### Bypassing Login

To bypass the login, you can use query params to assign JSON values directly to the User Configuration (see prior section for help determining appropriate values).

Examples:

* Skip the `initial-setup` screen: `/iris/?ui={"initial_setup_complete":true}`
* Set the username: `/iris?pusher={"username": "hello"}`
* Enable `snapcast`: `/iris/?snapcast={"enabled":true,"host":"my-host.com","port":"443"}`

This may alse be used to enable Spotify, by passing `?spotify={...}` with the valid token data. However, please be aware that this entails sharing a single login between sessions. Anybody with the pre-configured URL will be able to use the Spotify account in Iris. If the original user (who created the Spotify session) logs out, the token will **continue to work** for other users who access the page (the logout action does not invalidate the token for other users).

## Upgrading
You can upgrade from Iris under _Settings_ > _Upgrade_. This is in beta, and you will need to restart Mopidy for the upgrade to complete. Alternatively run `sudo pip install --upgrade Mopidy-Iris`.

If you're experiencing dependency issues, try installing without dependencies `sudo pip install --upgrade --no-deps Mopidy-Iris`.
