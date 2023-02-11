import React, { useEffect, useState } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { useParams } from 'react-router-dom';
import Loader from '../../components/Loader';
import Header from '../../components/Header';
import TrackList from '../../components/TrackList';
import { Grid } from '../../components/Grid';
import { List } from '../../components/List';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import Icon from '../../components/Icon';
import ErrorBoundary from '../../components/ErrorBoundary';
import {
  set,
  setWindowTitle,
  hideContextMenu,
} from '../../services/ui/actions';
import {
  getDirectory,
  playURIs,
} from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { arrayOf, sortItems, applyFilter } from '../../util/arrays';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { encodeUri, decodeUri } from '../../util/format';
import { makeLoadingSelector } from '../../util/selectors';
import ErrorMessage from '../../components/ErrorMessage';

const loadingSelector = makeLoadingSelector(['mopidy_library.(browse|lookup)']);

const Breadcrumbs = ({ uri }) => {
  let parent_uri = uri || null;
  if (!parent_uri.startsWith('file://')) return null;

  parent_uri = parent_uri.substring(0, parent_uri.lastIndexOf('/')).replace('file://', '');

  return (
    <h4>
      {decodeURI(parent_uri)}
    </h4>
  );
};

const Subdirectories = ({ items, view }) => {
  if (!items.length) return null;

  // Only define the link for directories; allows URILink to intelligently route based on type
  const link = (item) => (
    item.type === 'directory'
      ? `/library/browse/${encodeUri(item.uri)}/${encodeURIComponent(item.name)}`
      : null
  );

  if (view === 'list') {
    return (
      <List
        items={items}
        className="library-local-directory-list"
        getLink={link}
        nocontext
      />
    );
  }

  return (
    <Grid
      items={items}
      getLink={link}
      nocontext
    />
  );
};

const BrowseDirectory = () => {
  const dispatch = useDispatch();
  const [filter, setFilter] = useState('');
  const directoryInState = useSelector(({ mopidy }) => mopidy?.directory || {});
  const view = useSelector(({ ui }) => ui?.library_directory_view);
  const loading = useSelector(loadingSelector);

  let { uri, name } = useParams();
  uri = decodeUri(uri);
  name = decodeURIComponent(name);
  const uriMatcher = [uri, decodeURIComponent(uri)]; // Lenient matching due to encoding diffs
  const directory = directoryInState && uriMatcher.includes(directoryInState.uri)
    ? directoryInState
    : undefined;

  useEffect(() => {
    dispatch(setWindowTitle(i18n('library.browse_directory.title')));
    dispatch(getDirectory(uri));
  }, []);

  useEffect(() => {
    dispatch(getDirectory(uri));
  }, [uri]);

  const onViewChange = (value) => {
    dispatch(set({ library_directory_view: value }));
    dispatch(hideContextMenu());
  }

  const playAll = () => {
    if (!tracks || !tracks.length) return;

    dispatch(
      playURIs({
        uris: arrayOf('uri', sortItems(tracks, 'name')),
        from: {
          name: 'Browse',
          type: 'browse',
          uri,
        },
      })
    );
    dispatch(hideContextMenu());
  }

  const goBack = () => {
    dispatch(hideContextMenu());
    window.history.back();
  }

  if (!directory || (!directory.subdirectories && !directory.tracks)) {
    if (loading) {
      return <Loader body loading />;
    }
    return (
      <ErrorMessage type="not-found" title="Not found">
        <p>
          <I18n path="errors.uri_not_found" uri={uri} />
        </p>
      </ErrorMessage>
    );
  }

  let subdirectories = directory?.subdirectories;
  let tracks = directory?.tracks;
  subdirectories = sortItems(subdirectories, 'name');
  tracks = sortItems(tracks, 'name');
  if (filter && filter !== '') {
    subdirectories = applyFilter('name', filter, subdirectories);
    tracks = applyFilter('name', filter, tracks);
  }

  const view_options = [
    {
      value: 'thumbnails',
      label: i18n('fields.filters.thumbnails'),
    },
    {
      value: 'list',
      label: i18n('fields.filters.list'),
    },
  ];

  const options = (
    <>
      <FilterField
        initialValue={filter}
        handleChange={setFilter}
        onSubmit={() => dispatch(hideContextMenu())}
      />
      <DropdownField
        icon="visibility"
        name="View"
        value={view}
        valueAsLabel
        options={view_options}
        handleChange={onViewChange}
      />
      {tracks && (
        <Button
          onClick={playAll}
          noHover
          discrete
          tracking={{ category: 'Directory', action: 'Play' }}
        >
          <Icon name="play_circle_filled" />
          <I18n path="actions.play_all" />
        </Button>
      )}
      <Button
        onClick={goBack}
        tracking={{ category: 'Directory', action: 'Back' }}
        noHover
      >
        <Icon name="keyboard_backspace" />
        <I18n path="actions.back" />
      </Button>
    </>
  );

  return (
    <div className="view library-local-view">
      <Header options={options}>
        <Icon name="folder" type="material" />
        <div className="header__text">
          {name || i18n('library.browse_directory.title')}
          <Breadcrumbs uri={uri} />
        </div>
      </Header>
      <section className="content-wrapper">
        <ErrorBoundary>

          <Subdirectories items={subdirectories} view={view} />

          <TrackList
            context={{
              uri,
              name: 'Browse',
              type: 'browse',
            }}
            tracks={tracks}
            className="library-local-track-list"
          />

        </ErrorBoundary>
      </section>
    </div>
  );
}

export default BrowseDirectory;
