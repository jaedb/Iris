import React, { useState, useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useParams } from 'react-router-dom';
import sanitizeHtml from 'sanitize-html';
import ErrorMessage from '../components/ErrorMessage';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import LinksSentence from '../components/LinksSentence';
import Loader from '../components/Loader';
import FollowButton from '../components/Fields/FollowButton';
import { nice_number } from '../components/NiceNumber';
import { Dater } from '../components/Dater';
import ContextMenuTrigger from '../components/ContextMenu/ContextMenuTrigger';
import { SourceIcon } from '../components/Icon';
import DropdownField from '../components/Fields/DropdownField';
import FilterField from '../components/Fields/FilterField';
import { i18n, I18n } from '../locale';
import { loadAlbum } from '../services/core/actions';
import { playURIs } from '../services/mopidy/actions';
import { getAlbum } from '../services/lastfm/actions';
import {
  setSort,
  setWindowTitle,
  showContextMenu,
  hideContextMenu,
} from '../services/ui/actions';
import { uriSource } from '../util/helpers';
import Button from '../components/Button';
import { makeItemSelector, makeSortSelector } from '../util/selectors';
import { applyFilter, sortItems } from '../util/arrays';
import { decodeUri, formatContext } from '../util/format';

const SORT_KEY = 'album_tracks';

const Album = () => {
  const dispatch = useDispatch();
  const { uri: encodedUri } = useParams();
  const uri = decodeUri(encodedUri);

  const itemSelector = makeItemSelector(uri);
  const album = useSelector(itemSelector);

  const sortSelector = makeSortSelector(SORT_KEY, 'disc_track');
  const [sortField, sortReverse] = useSelector(sortSelector);
  const slim_mode = useSelector((state) => state.ui.slim_mode);

  const [filter, setFilter] = useState('');

  let tracks = album?.tracks || [];
  if (sortField && tracks) tracks = sortItems(tracks, sortField, sortReverse);
  if (filter && filter !== '') tracks = applyFilter('name', filter, tracks);

  useEffect(
    () => {
      if (uri) dispatch(loadAlbum(uri, { full: true }));
    },
    [uri],
  );

  useEffect(() => {
    if (album?.artists && album?.wiki === undefined) {
      dispatch(getAlbum(album.uri, album.artists[0].name, album.name));
    }
  }, [album]);

  useEffect(() => {
    if (album) {
      setWindowTitle(i18n('album.title_window', {
        name: album.name,
        artist: album.artists ? album.artists.map((artist) => artist.name).join(', ') : '',
      }));
    } else {
      setWindowTitle(i18n('album.title'));
    }
  }, [album]);

  if (!album?.name && album?.loading) {
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

  const play = () => {
    dispatch(playURIs({
      uris: [uri],
      from: {
        uri,
        name: album?.name,
        type: 'album',
      },
    }));
  };

  const handleContextMenu = (e) => dispatch(
    showContextMenu({
      e,
      item: album,
      type: 'album',
    }),
  );

  const onChangeSort = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    dispatch(setSort(SORT_KEY, field, reverse));
    dispatch(hideContextMenu());
  };

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
        <Thumbnail
          size="large"
          images={album.images}
          type="album"
          loading={album.loading}
          canZoom
          glow
        />
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
          onClick={play}
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
        <ContextMenuTrigger onTrigger={handleContextMenu} />
      </div>

      <section className="list-wrapper">
        <h4 className="no-bottom-margin">
          <I18n path="album.tracks.title" />
          <div className="actions-wrapper">
            <FilterField
              initialValue={filter}
              handleChange={setFilter}
              onSubmit={hideContextMenu}
            />
            <DropdownField
              icon="swap_vert"
              name="Sort"
              value={sortField}
              valueAsLabel
              options={sort_options}
              selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
              handleChange={onChangeSort}
            />
          </div>
        </h4>
        <TrackList
          context={formatContext({ ...album, can_drag: true })}
          className="album-track-list"
          tracks={tracks}
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

export default Album;
