
import React, { createRef } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';
import {
  uriSource,
  uriType,
  getFromUri,
  buildLink,
  isLoading,
  throttle,
} from '../util/helpers';
import {
  arrayOf,
  sortItems,
  indexToArray,
} from '../util/arrays';
import Link from './Link';
import Icon from './Icon';
import Loader from './Loader';
import URILink from './URILink';

import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as spotifyActions from '../services/spotify/actions';

class ContextMenu extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      submenu: null,
    };
    this.handleScroll = throttle(this.handleScroll.bind(this), 50);
    this.handleMouseDown = this.handleMouseDown.bind(this);
    this.handleTouchStart = this.handleTouchStart.bind(this);
  }

  componentDidMount = () => {
    window.addEventListener('scroll', this.handleScroll, false);
    window.addEventListener('mousedown', this.handleMouseDown, false);
    window.addEventListener('touchstart', this.handleTouchStart, false);
  }

  componentWillUnmount = () => {
    window.removeEventListener('scroll', this.handleScroll, false);
    window.removeEventListener('mousedown', this.handleMouseDown, false);
    window.removeEventListener('touchstart', this.handleTouchStart, false);
  }

  componentDidUpdate = (prevProps) => {
    const {
      menu: prevMenu,
      tracks: prevTracks,
    } = prevProps;
    const {
      menu,
      tracks,
      lastfm_authorized,
      spotify_available,
      spotifyActions,
      lastfmActions,
    } = this.props;

    // if we've been given a menu object (ie activated) when we didn't have one prior
    if (!prevMenu && menu) {
      this.setState({ submenu: null });

      const context = this.getContext(this.props);

      // if we're able to be in the library, run a check
      if (spotify_available && context.source === 'spotify') {
        switch (menu.context) {
          case 'artist':
          case 'album':
          case 'playlist':
          case 'editable-playlist':
          case 'track':
          case 'playlist-track':
          case 'editable-playlist-track':
          case 'queue-track':
            spotifyActions.following(menu.items[0].uri);
            break;
          default:
            break;
        }
      }

      // if we're able to be in the LastFM library, run a check
      if (lastfm_authorized && context.is_track && context.items_count == 1) {
        if (menu.items[0].uri && prevTracks[menu.items[0].uri] !== undefined && tracks[menu.items[0].uri].userloved === undefined) {
          lastfmActions.getTrack(menu.items[0].uri);
        }
      }
    }
  }

  getContext(props = this.props) {
    const {
      menu,
    } = props;
    const context = {
      name: null,
      nice_name: 'Unknown',
      is_track: false,
    };

    if (menu && menu.context) {
      context.name = menu.context;
      context.nice_name = menu.context;

      // handle ugly labels
      switch (menu.context) {
        case 'playlist':
        case 'editable-playlist':
          context.nice_name = 'playlist';
          break;

        case 'track':
        case 'queue-track':
        case 'playlist-track':
        case 'editable-playlist-track':
          context.nice_name = 'track';
          context.is_track = true;
          break;
      }

      // Consider the object(s) themselves
      // We can only really accommodate the first item. The only instances where
      // there is multiple is tracklists, when they're all of the same source (except search?)
      if (menu.items && menu.items.length > 0) {
        const item = menu.items[0];
        context.item = item;
        context.items_count = menu.items.length;
        context.source = uriSource(item.uri);
        context.type = uriType(item.uri);
        context.in_library = this.inLibrary(item);
        context.is_loved = this.isLoved(item);
      }
    }

    return context;
  }

  handleScroll = () => {
    const { menu, uiActions: { hideContextMenu } } = this.props;

    if (menu) hideContextMenu();
  }

  handleMouseDown = (e) => {
    const { menu, uiActions: { hideContextMenu } } = this.props;

    if (menu && $(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0) {
      hideContextMenu();
    }
  }

  handleTouchStart = (e) => {
    const { menu, uiActions: { hideContextMenu } } = this.props;

    if (menu && $(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0) {
      hideContextMenu();
    }
  }

  inLibrary = ({ uri } = {}) => {
    const {
      spotify_library_artists,
      spotify_library_albums,
      spotify_library_playlists,
      spotify_library_tracks,
    } = this.props;
    if (!uri) return false;

    switch (uriType(uri)) {
      case 'artist':
        return (spotify_library_artists && spotify_library_artists.indexOf(uri) > -1);
      case 'album':
        return (spotify_library_albums && spotify_library_albums.indexOf(uri) > -1);
      case 'playlist':
        return (spotify_library_playlists && spotify_library_playlists.indexOf(uri) > -1);
      case 'track':
        return (spotify_library_tracks && spotify_library_tracks.indexOf(uri) > -1);
      default:
        return false;
    }
  }

  /**
   * TODO: Currently the select track keys are the only details available. We need
   * the actual track object reference (including name and artists) to getTrack from LastFM
   * */
  isLoved = ({ uri } = {}) => {
    const {
      tracks,
    } = this.props;

    if (!uri) return false;
    if (tracks[uri] === undefined) return false;

    const track = tracks[uri];
    return (track.userloved !== undefined && track.userloved == '1');
  }

  canBeInLibrary = () => {
    const { spotify_available, menu: { items } } = this.props;
    if (!spotify_available) return false;
    return (uriSource(items[0].uri) === 'spotify');
  }

  toggleInLibrary = (e, in_library) => {
    const {
      uiActions: {
        hideContextMenu,
      },
      spotifyActions: {
        following,
      },
      menu: {
        items,
      } = {},
    } = this.props;

    hideContextMenu();
    if (in_library) {
      following(items[0].uri, 'DELETE');
    } else {
      following(items[0].uri, 'PUT');
    }
  }

  playQueueItem = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        changeTrack,
      },
      menu: {
        items,
      } = {},
    } = this.props;

    hideContextMenu();
    changeTrack(items[0].tlid);
  }

  removeFromQueue = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        removeTracks,
      },
      menu: {
        items,
      } = {},
    } = this.props;

    hideContextMenu();
    removeTracks(arrayOf('tlid', items));
  }

  playURIs = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        playURIs,
      },
      menu: {
        uris,
        tracklist_uri,
      } = {},
    } = this.props;

    hideContextMenu();
    playURIs(uris, tracklist_uri);
  }

  playPlaylist = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        playPlaylist,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    playPlaylist(uris[0]);
  }

  enqueuePlaylist = (e, play_next = false) => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        enqueuePlaylist,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    enqueuePlaylist(uris[0], play_next);
  }

  shufflePlayPlaylist = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        playPlaylist,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    playPlaylist(uris[0], true);
  }

  playArtistTopTracks = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      spotifyActions: {
        playArtistTopTracks,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    playArtistTopTracks(uris[0]);
  }

  addToQueue = (e, play_next = false) => {
    const {
      uiActions: {
        hideContextMenu,
      },
      mopidyActions: {
        enqueueURIs,
      },
      menu: {
        uris,
        tracklist_uri,
      } = {},
    } = this.props;

    hideContextMenu();
    enqueueURIs(uris, tracklist_uri, play_next);
  }

  addTracksToPlaylist = (e, playlist_uri) => {
    const {
      uiActions: {
        hideContextMenu,
      },
      coreActions: {
        addTracksToPlaylist,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    addTracksToPlaylist(playlist_uri, uris);
  }

  toggleLoved = (e, is_loved) => {
    const {
      uiActions: {
        hideContextMenu,
      },
      lastfmActions: {
        unloveTrack,
        loveTrack,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    if (is_loved) {
      unloveTrack(uris[0]);
    } else {
      loveTrack(uris[0]);
    }
  }

  unloveTrack = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      lastfmActions: {
        unloveTrack,
      },
      menu: {
        items,
      } = {},
    } = this.props;

    hideContextMenu();
    unloveTrack(items[0]);
  }

  removeFromPlaylist = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      coreActions: {
        removeTracksFromPlaylist,
      },
      menu: {
        tracklist_uri,
        indexes,
      } = {},
    } = this.props;

    hideContextMenu();
    removeTracksFromPlaylist(tracklist_uri, indexes);
  }

  deletePlaylist = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      coreActions: {
        deletePlaylist,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    deletePlaylist(uris[0]);
  }

  startRadio = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      pusherActions: {
        startRadio,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    hideContextMenu();
    startRadio(uris);
  }

  goToRecommendations = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      menu: {
        items,
      } = {},
      history: {
        push,
      },
    } = this.props;

    hideContextMenu();
    push(`/discover/recommendations/${arrayOf('uri', items).join(',')}`);
  }

  goToArtist = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      menu: {
        items,
      } = {},
      history: {
        push,
      },
    } = this.props;

    if (!items || items.length <= 0 || !items[0].artists_uris || items[0].artists_uris.length <= 0) {
      return null;
    }
    hideContextMenu();

    // note: we can only go to one artist (even if this item has multiple artists, just go to the first one)
    push(buildLink(items[0].artists_uris[0]));
  }

  goToUser = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      menu: {
        items,
      } = {},
      history: {
        push,
      },
    } = this.props;

    if (!items || items.length <= 0 || !items[0].user_uri) return null;
    hideContextMenu();
    push(buildLink(items[0].user_uri));
  }

  goToTrack = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      menu: {
        uris,
      } = {},
      history: {
        push,
      },
    } = this.props;

    if (!uris) return null;
    hideContextMenu();
    push(buildLink(uris[0]));
  }

  copyURIs = () => {
    const {
      uiActions: {
        hideContextMenu,
        createNotification,
      },
      menu: {
        uris,
      } = {},
    } = this.props;

    const temp = $('<input>');
    $('body').append(temp);
    temp.val(uris.join(',')).select();
    document.execCommand('copy');
    temp.remove();

    createNotification({ content: `Copied ${uris.length} URIs` });
    hideContextMenu();
  }

  renderTitle = () => {
    const {
      uiActions: {
        hideContextMenu,
        setSelectedTracks,
      },
      queue_metadata,
      menu: {
        title,
      },
    } = this.props;

    const context = this.getContext();

    if (context.items_count > 1) {
      return (
        <div className="context-menu__title">
          <div className="context-menu__title__text">
            {`${context.items_count} ${context.nice_name}${context.items_count > 1 ? 's' : ''} selected`}
            <span
              className="context-menu__title__deselect"
              onClick={() => {
                setSelectedTracks([]);
                hideContextMenu();
              }}>
                <Icon name="close" />
              </span>
          </div>
        </div>
      );
    }

    if (context.items_count === 1 && context.name === 'queue-track' && context.item !== undefined) {
      if (queue_metadata[`tlid_${context.item.tlid}`] !== undefined) {
        const metadata = queue_metadata[`tlid_${context.item.tlid}`];

        if (metadata.added_from && metadata.added_by) {
          const type = (metadata.added_from ? uriType(metadata.added_from) : null);

          switch (type) {
            case 'discover':
              var link = <URILink type="recommendations" uri={getFromUri('seeds', metadata.added_from)}>discover</URILink>;
              break;

            case 'browse':
              var link = <URILink type="browse" uri={metadata.added_from.replace('iris:browse:', '')}>browse</URILink>;
              break;

            case 'search':
              var link = <URILink type="search" uri={metadata.added_from.replace('iris:', '')}>search</URILink>;
              break;

            default:
              var link = <URILink type={type} uri={metadata.added_from}>{type}</URILink>;
          }

          return (
            <div className="context-menu__title">
              <div className="context-menu__title__text">
                {`${metadata.added_by} added from `}
                {link}
              </div>
            </div>
          );
        }
      }
    }

    if (context.name === 'custom') {
      if (!title) return null;

      return (
        <div className="context-menu__title">
          <div className="context-menu__title__text">
            {title}
          </div>
        </div>
      );
    }

    return null;
  }

  setSubmenu = (name) => {
    const { submenu } = this.state;
    const {
      spotify_available,
      spotify_library_playlists_loaded_all,
      mopidy_library_playlists_loaded_all,
      spotifyActions,
      mopidyActions,
    } = this.props;
    if (submenu !== name && name === 'add-to-playlist') {
      if (spotify_available && !spotify_library_playlists_loaded_all) spotifyActions.getLibraryPlaylists();
      if (!mopidy_library_playlists_loaded_all) mopidyActions.getLibraryPlaylists();
    }

    this.setState({ submenu: name });
  }

  closeSubmenu = () => this.setState({ submenu: null });

  renderSubmenu = () => {
    const { submenu } = this.state;
    const {
      playlists: playlistsIndex,
      processes,
    } = this.props;

    let list = null;
    let isLoading = false;

    if (submenu === 'add-to-playlist') {
      let playlists = indexToArray(playlistsIndex);
      playlists = playlists.filter((playlist) => playlist.can_edit);
      playlists = sortItems(playlists, 'name');

      if (processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR && processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status === 'running') {
        isLoading = true;
      }

      list = (
        <span className="context-menu__item">
          <span className="context-menu__item mid_grey-text">
            <span className="context-menu__item__link context-menu__item__link--inactive">
              No writable playlists
            </span>
          </span>
        </span>
      );
      if (playlists.length) {
        list = playlists.map((playlist) => (
          <span className="context-menu__item" key={playlist.uri}>
            <a
              className="context-menu__item__link"
              onClick={(e) => this.addTracksToPlaylist(e, playlist.uri)}
            >
              <span className="context-menu__item__label">
                {playlist.name}
              </span>
            </a>
          </span>
        ));
      }
    }

    return (
      <div className="context-menu__section context-menu__section--submenu">
        <div className="context-menu__item">
          <a
            className="context-menu__item__link context-menu__item__link--close-submenu"
            onClick={this.closeSubmenu}
          >
            <span className="context-menu__item__label">
              <Icon name="arrow_back" />
              <span> Back</span>
            </span>
          </a>
        </div>
        {list}
        {isLoading && (
          <div className="context-menu__item">
            <Loader className="context-menu__item" mini loading />
          </div>
        )}
      </div>
    );
  }

  renderItems = () => {
    const {
      lastfm_authorized,
      spotify_available,
      load_queue,
    } = this.props;
    const context = this.getContext();

    const play_uris = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.playURIs}>
          <span className="context-menu__item__label">Play</span>
        </a>
      </div>
    );

    const play_playlist = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.playPlaylist}>
          <span className="context-menu__item__label">Play</span>
        </a>
      </div>
    );

    const shuffle_play_playlist = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.shufflePlayPlaylist}>
          <span className="context-menu__item__label">Shuffle play</span>
        </a>
      </div>
    );

    const play_queue_item = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.playQueueItem}>
          <span className="context-menu__item__label">Play</span>
        </a>
      </div>
    );

    const play_uris_next = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={(e) => this.addToQueue(e, true)}>
          <span className="context-menu__item__label">Play next</span>
        </a>
      </div>
    );

    const play_artist_top_tracks = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.playArtistTopTracks}>
          <span className="context-menu__item__label">Play top tracks</span>
        </a>
      </div>
    );

    const add_to_queue = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.addToQueue}>
          <span className="context-menu__item__label">Add to queue</span>
        </a>
      </div>
    );

    const add_playlist_to_queue = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.enqueuePlaylist}>
          <span className="context-menu__item__label">Add to queue</span>
        </a>
      </div>
    );

    const play_playlist_next = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={(e) => this.enqueuePlaylist(e, true)}>
          <span className="context-menu__item__label">Play next</span>
        </a>
      </div>
    );

    const add_to_playlist = (
      <div className="context-menu__item context-menu__item--has-submenu">
        <a className="context-menu__item__link" onClick={() => this.setSubmenu('add-to-playlist')}>
          <span className="context-menu__item__label">Add to playlist</span>
          <Icon className="submenu-icon" name="arrow_forward" />
        </a>
      </div>
    );

    let toggle_in_library = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={(e) => this.toggleInLibrary(e, context.in_library)}>
          <span className="context-menu__item__label">
            {context.in_library ? 'Remove from library' : 'Add to library'}
          </span>
        </a>
      </div>
    );

    if (!spotify_available) {
      toggle_in_library = null;
    } else if (
      isLoading(
        load_queue,
        [
          'spotify_me/tracks/contains',
          'spotify_me/playlists/contains',
          'spotify_me/albums/contains',
          'spotify_me/artists/contains'
        ]
      )
    ) {
      toggle_in_library = (
        <div className="context-menu__item">
          <a className="context-menu__item__link">
            <span className="context-menu__item__label mid_grey-text">
							Add to library
            </span>
          </a>
        </div>
      );
    } else {
      toggle_in_library = (
        <div className="context-menu__item">
          <a className="context-menu__item__link" onClick={(e) => this.toggleInLibrary(e, context.in_library)}>
            <span className="context-menu__item__label">
              {context.in_library ? 'Remove from library' : 'Add to library'}
            </span>
          </a>
        </div>
      );
    }

    if (!lastfm_authorized) {
      var toggle_loved = null;
    } else if (isLoading(load_queue, ['lastfm_track.getInfo'])) {
      var toggle_loved = (
        <div className="context-menu__item">
          <a className="context-menu__item__link">
            <span className="context-menu__item__label mid_grey-text">
							Love track
            </span>
          </a>
        </div>
      );
    } else {
      var toggle_loved = (
        <div className="context-menu__item">
          <a className="context-menu__item__link" onClick={(e) => this.toggleLoved(e, context.is_loved)}>
            <span className="context-menu__item__label">
              {context.is_loved ? 'Unlove track' : 'Love track'}
            </span>
          </a>
        </div>
      );
    }

    const go_to_artist = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.goToArtist}>
          <span className="context-menu__item__label">Go to artist</span>
        </a>
      </div>
    );

    const go_to_user = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.goToUser}>
          <span className="context-menu__item__label">Go to user</span>
        </a>
      </div>
    );

    const go_to_track = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.goToTrack}>
          <span className="context-menu__item__label">Track info</span>
        </a>
      </div>
    );

    const go_to_recommendations = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.goToRecommendations}>
          <span className="context-menu__item__label">Discover similar</span>
        </a>
      </div>
    );

    const start_radio = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.startRadio}>
          <span className="context-menu__item__label">Start radio</span>
        </a>
      </div>
    );

    const remove_from_queue = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.removeFromQueue}>
          <span className="context-menu__item__label">Remove</span>
        </a>
      </div>
    );

    const remove_from_playlist = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.removeFromPlaylist}>
          <span className="context-menu__item__label">Remove</span>
        </a>
      </div>
    );

    const edit_playlist = (
      <div className="context-menu__item">
        <Link className="context-menu__item__link" to={`/playlist/${context.item.uri}/edit`}>
          <span className="context-menu__item__label">Edit</span>
        </Link>
      </div>
    );

    const delete_playlist = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.deletePlaylist}>
          <span className="context-menu__item__label">Delete</span>
        </a>
      </div>
    );

    const copy_uris = (
      <div className="context-menu__item">
        <a className="context-menu__item__link" onClick={this.copyURIs}>
          <span className="context-menu__item__label">
            {`Copy URI ${context.items_count > 1 ? 's' : ''}`}
          </span>
        </a>
      </div>
    );

    switch (context.name) {
      case 'album':
        return (
          <div>
            {play_uris}
            {play_uris_next}
            {add_to_queue}
            {this.canBeInLibrary() && <div className="context-menu__divider" />}
            {this.canBeInLibrary() && toggle_in_library}
            <div className="context-menu__divider" />
            {go_to_artist}
            {copy_uris}
          </div>
        );
      case 'artist':
        return (
          <div>
            {context.source === 'spotify' && play_artist_top_tracks}
            {context.source === 'spotify' && start_radio}
            {this.canBeInLibrary() && <div className="context-menu__divider" />}
            {this.canBeInLibrary() && toggle_in_library}
            <div className="context-menu__divider" />
            {context.source === 'spotify' && go_to_recommendations}
            {copy_uris}
          </div>
        );
      case 'playlist':
        return (
          <div>
            {play_playlist}
            {play_playlist_next}
            {shuffle_play_playlist}
            {add_playlist_to_queue}
            {this.canBeInLibrary() && <div className="context-menu__divider" /> }
            {this.canBeInLibrary() && toggle_in_library}
            <div className="context-menu__divider" />
            {context.source === 'spotify' && go_to_user}
            {copy_uris}
            {context.items_count === 1 && context.item.can_edit && (
              <div>
                <div className="context-menu__divider" />
                {edit_playlist}
                {delete_playlist}
              </div>
            )}
          </div>
        );
      case 'queue-track':
        return (
          <div>
            {context.items_count === 1 && play_queue_item}
            <div className="context-menu__divider" />
            {add_to_playlist}
            {this.canBeInLibrary() && toggle_in_library}
            {toggle_loved}
            <div className="context-menu__divider" />
            {context.source === 'spotify' && context.items_count <= 5 && go_to_recommendations}
            {context.items_count === 1 && go_to_track}
            {copy_uris}
            <div className="context-menu__divider" />
            {remove_from_queue}
          </div>
        );
      case 'editable-playlist-track':
        return (
          <div>
            {play_uris}
            {play_uris_next}
            {add_to_queue}
            {context.source === 'spotify' && context.items_count === 1 && start_radio}
            <div className="context-menu__divider" />
            {add_to_playlist}
            {this.canBeInLibrary() && toggle_in_library}
            {toggle_loved}
            <div className="context-menu__divider" />
            {context.source === 'spotify' && context.items_count <= 5 && go_to_recommendations}
            {context.items_count === 1 && go_to_track}
            {copy_uris}
            <div className="context-menu__divider" />
            {remove_from_playlist}
          </div>
        );
      default:
        return (
          <div>
            {play_uris}
            {play_uris_next}
            {add_to_queue}
            {context.source === 'spotify' && context.items_count === 1 && start_radio}
            <div className="context-menu__divider" />
            {add_to_playlist}
            {this.canBeInLibrary() && toggle_in_library}
            {toggle_loved}
            <div className="context-menu__divider" />
            {context.source === 'spotify' && context.items_count <= 5 && go_to_recommendations}
            {context.items_count === 1 && go_to_track}
            <div className="context-menu__divider" />
            {copy_uris}
          </div>
        );
    }
  }

  render() {
    const {
      menu,
      uiActions: {
        hideContextMenu,
      },
    } = this.props;
    const { submenu } = this.state;

    if (!menu) return null;

    const style = {
      left: menu.position_x,
      top: menu.position_y,
    };
    const height = 200; // TODO: use jquery to detect height
    let className = `context-menu ${menu.context}`;
    if (submenu) className += ' context-menu--submenu-expanded';
    if (menu.closing) className += ' context-menu--closing';

    if (menu.position_x > (window.innerWidth - 174)) {
      style.left = 'auto';
      style.right = 10;
    }

    if (menu.position_y > (window.innerHeight - height)) {
      style.top = 'auto';
      style.bottom = 10;
    }

    return (
      <div id="context-menu" className={className} style={style}>
        <div className="context-menu__section context-menu__section--items">
          {this.renderTitle()}
          {menu.context === 'custom' ? menu.options : this.renderItems()}
        </div>
        {this.renderSubmenu()}
        <div className="context-menu__background" onClick={hideContextMenu} />
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  menu: state.ui.context_menu,
  load_queue: state.ui.load_queue,
  processes: state.ui.processes,
  current_track: state.core.current_track,
  current_tracklist: state.core.current_tracklist,
  queue_metadata: state.core.queue_metadata,
  spotify_available: state.spotify.access_token,
  spotify_library_playlists: state.spotify.library_playlists,
  spotify_library_playlists_loaded_all: state.spotify.library_playlists_loaded_all,
  spotify_library_artists: state.spotify.library_artists,
  spotify_library_albums: state.spotify.library_albums,
  spotify_library_tracks: state.spotify.library_tracks,
  mopidy_library_playlists: state.mopidy.library_playlists,
  mopidy_library_playlists_loaded_all: state.mopidy.library_playlists_loaded_all,
  mopidy_library_artists: state.mopidy.library_artists,
  mopidy_library_albums: state.mopidy.library_albums,
  playlists: state.core.playlists,
  tracks: state.core.tracks,
  lastfm_authorized: state.lastfm.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(withRouter(ContextMenu));
