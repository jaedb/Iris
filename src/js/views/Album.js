import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import sanitizeHtml from 'sanitize-html';

import ErrorMessage from '../components/ErrorMessage';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import LinksSentence from '../components/LinksSentence';
import Loader from '../components/Loader';
import FollowButton from '../components/Fields/FollowButton';
import { nice_number } from '../components/NiceNumber';
import { Dater } from '../components/Dater';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import Icon, { SourceIcon } from '../components/Icon';
import DropdownField from '../components/Fields/DropdownField';
import FilterField from '../components/Fields/FilterField';
import { i18n, I18n } from '../locale';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import {
  uriSource,
  sourceIcon,
} from '../util/helpers';
import Button from '../components/Button';
import { makeLoadingSelector, makeItemSelector } from '../util/selectors';
import { applyFilter, sortItems } from '../util/arrays';
import { trackEvent } from '../components/Trackable';
import { encodeUri, decodeUri } from '../util/format';

class Album extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      filter: '',
    };
  }

  componentDidMount = () => {
    const {
      uri,
      album,
      coreActions: {
        loadAlbum,
      },
      lastfmActions: {
        getAlbum,
      },
    } = this.props;

    this.setWindowTitle();
    loadAlbum(uri, { full: true });

    if (album) {
      if (album.artists && album.wiki === undefined) {
        getAlbum(album.uri, album.artists[0].name, album.name);
      }
    }
  }

  componentDidUpdate = ({
    uri: prevUri,
    album: prevAlbum,
  }) => {
    const {
      uri,
      album,
      coreActions: {
        loadAlbum,
      },
      lastfmActions: {
        getAlbum,
      },
    } = this.props;

    if (uri !== prevUri) {
      loadAlbum(uri, { full: true });
    }

    // We have just received our full album or our album artists
    if ((!prevAlbum && album)) {
      if (album.artists && album.wiki === undefined) {
        getAlbum(album.uri, album.artists[0].name, album.name);
      }
    }

    if (!prevAlbum && album) this.setWindowTitle(album);
  }

  setWindowTitle = (album = this.props.album) => {
    const { uiActions: { setWindowTitle } } = this.props;

    if (album) {
      setWindowTitle(i18n('album.title_window', {
        name: album.name,
        artist: album.artists?.map((artist) => artist.name).join(),
      }));
    } else {
      setWindowTitle(i18n('album.title'));
    }
  }

  handleContextMenu = (e) => {
    const { album, uri, uiActions: { showContextMenu } } = this.props;

    showContextMenu({
      e,
      context: 'album',
      items: [album],
      uris: [uri],
    });
  }

  loadMore = () => {
    const {
      spotifyActions: {
        getMore,
      },
      album: {
        uri,
        name,
        tracks_more,
      } = {},
    } = this.props;

    getMore(
      tracks_more, {
        parent_type: 'album',
        parent_key: uri,
        records_type: 'track',
      },
      null,
      {
        album: {
          uri,
          name,
        },
      },
    );
  }

  play = () => {
    const { uri, mopidyActions: { playURIs } } = this.props;
    playURIs([uri], uri);
  }

  onChangeSort = (value) => {
    const {
      sort,
      sort_reverse,
      uiActions: {
        set,
        hideContextMenu,
      },
    } = this.props;

    let reverse = false;
    if (value !== null && sort === value) {
      reverse = !sort_reverse;
    }

    set({
      album_tracks_sort_reverse: reverse,
      album_tracks_sort: value,
    });
    hideContextMenu();
    trackEvent({ category: 'Album', action: 'SortTracks', label: `${value} ${reverse ? 'DESC' : 'ASC'}` });
  }

  inLibrary = () => {
    const { uri } = this.props;
    const library = `${uriSource(uri)}_library_albums`;
    return (this.props[library] && this.props[library].indexOf(this.props.uri) > -1);
  }

  render = () => {
    const {
      uri,
      album,
      loading,
      slim_mode,
      sort,
      sort_reverse,
    } = this.props;
    let {
      album: {
        tracks,
      } = {},
    } = this.props;
    const {
      filter,
    } = this.state;

    if (loading) {
      return <Loader body loading />;
    }
    if (!album) {
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            {i18n('errors.uri_not_found', { uri })}
          </p>
        </ErrorMessage>
      );
    }

    if (sort && tracks) {
      tracks = sortItems(tracks, sort, sort_reverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    const sort_options = [
      {
        value: 'disc_track',
        label: i18n('album.tracks.sort.disc_track'),
      },
      {
        value: 'name',
        label: i18n('album.tracks.sort.name'),
      },
    ];

    return (
      <div className="view album-view content-wrapper preserve-3d">
        <div className="thumbnail-wrapper">
          <Thumbnail size="large" glow canZoom images={album.images} type="album" />
        </div>

        <div className="title">
          <h1>{album.name}</h1>

          <ul className="details details--one-line">
            {!slim_mode ? (
              <li className="source">
                <SourceIcon uri={album.uri} />
              </li>
            ) : null}
            {album.artists && album.artists.length > 0 ? (
              <li>
                <LinksSentence items={album.artists} type="artist" />
              </li>
            ) : null}
            {album.release_date ? (
              <li>
                <Dater type="date" data={album.release_date} />
              </li>
            ) : null}
            {album.tracks ? (
              <li>
                {i18n(
                  'specs.tracks',
                  { count: album.tracks.length },
                )}
              </li>
            ) : null}
            {!slim_mode && album.tracks ? (
              <li>
                <Dater type="total-time" data={album.tracks} />
              </li>
            ) : null}
            {!slim_mode && album.play_count ? (
              <li>
                {i18n(
                  'specs.plays',
                  { count: nice_number(album.play_count) },
                )}
              </li>
            ) : null}
            {!slim_mode && album.listeners ? (
              <li>
                {i18n(
                  'specs.listeners',
                  { count: nice_number(album.listeners) },
                )}
              </li>
            ) : null}
          </ul>
        </div>

        <div className="actions">
          <Button
            type="primary"
            onClick={this.play}
            tracking={{ category: 'Album', action: 'Play' }}
          >
            <I18n path="actions.play" />
          </Button>
          {uriSource(uri) === 'spotify' && (
            <FollowButton
              uri={uri}
              is_following={album.in_library}
            />
          )}
          <ContextMenuTrigger onTrigger={this.handleContextMenu} />
        </div>

        <section className="list-wrapper">
          <h4 className="no-bottom-margin">
            <I18n path="album.tracks.title" />
            <div className="actions-wrapper">
              <FilterField
                initialValue={filter}
                handleChange={(value) => this.setState({ filter: value })}
                onSubmit={() => uiActions.hideContextMenu()}
              />
              <DropdownField
                icon="swap_vert"
                name="Sort"
                value={sort}
                valueAsLabel
                options={sort_options}
                selected_icon={sort ? (sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={this.onChangeSort}
              />
            </div>
          </h4>
          <TrackList
            className="album-track-list"
            tracks={tracks}
            track_context="album"
            uri={album.uri}
          />
        </section>

        {album.wiki ? (
          <section className="wiki">
            <h4 className="wiki__title">{i18n('album.wiki.title')}</h4>
            <div className="wiki__text">
              <p dangerouslySetInnerHTML={{ __html: sanitizeHtml(album.wiki) }} />
              <br />
              <div className="mid_grey-text">
                <I18n path="album.wiki.published" date={album.wiki_publish_date} />
              </div>
            </div>
          </section>
        ) : null}
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeUri(ownProps.match.params.uri);
  const itemSelector = makeItemSelector(uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`, '^((?!contains).)*$', '^((?!me\/albums).)*$', '^((?!followers).)*$']);
  return {
    uri,
    slim_mode: state.ui.slim_mode,
    theme: state.ui.theme,
    album: itemSelector(state),
    loading: loadingSelector(state),
    spotify_library_albums: state.spotify.library_albums,
    local_library_albums: state.mopidy.library_albums,
    spotify_authorized: state.spotify.authorization,
    sort: (state.ui.album_tracks_sort ? state.ui.album_tracks_sort : 'disc_track'),
    sort_reverse: (!!state.ui.album_tracks_sort_reverse),
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
});

export {
  Album,
};

export default connect(
  mapStateToProps,
  mapDispatchToProps,
)(Album);
