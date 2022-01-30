import React, { useEffect, useState, useRef } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';
import { compact } from 'lodash';
import {
  uriSource,
  uriType,
  getFromUri,
  buildLink,
  isLoading,
  throttle,
  titleCase,
} from '../../util/helpers';
import {
  arrayOf,
  sortItems,
} from '../../util/arrays';
import Link from '../Link';
import Icon from '../Icon';
import Loader from '../Loader';
import URILink from '../URILink';

import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as lastfmActions from '../../services/lastfm/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { I18n } from '../../locale';
import { encodeUri } from '../../util/format';
import {
  makeProcessProgressSelector,
  makeProvidersSelector,
  makeLibrarySelector,
} from '../../util/selectors';
import AddedFrom from '../AddedFrom';
import { ContextMenuItems } from './ContextMenuItems';
import PlaylistSubmenu from './PlaylistSubmenu';

const processKeys = [
  'MOPIDY_GET_LIBRARY_PLAYLISTS',
  'SPOTIFY_GET_LIBRARY_PLAYLISTS',
];

const Title = ({
  context_menu: {
    title,
    item,
    items,
    type,
  },
}) => {
  if (items) {
    return (
      <div className="context-menu__title">
        <div className="context-menu__title__text">
          {`${items.length} ${type} selected`}
          <span
            className="context-menu__title__deselect"
            onClick={() => {
              // TODO: reset selected
            }}
          >
            <Icon name="close" />
          </span>
        </div>
      </div>
    );
  }

  if (item) {
    return (
      <div className="context-menu__title">
        <AddedFrom
          from={item?.added_from}
          by={item?.added_by}
          className="context-menu__title__text"
          inline
        />
      </div>
    );
  }

  if (type === 'custom') {
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
};

const ContextMenu = ({
  context_menu,
  loading_progress,  // to move to submenu
  playlists, // to move to submenu
  providers, // to move to submenu
  uiActions: {
    hideContextMenu,
  },
}) => {
  const ref = useRef();
  const [submenu, setSubmenu] = useState(false);

  const onScroll = () => hideContextMenu();

  const onClick = (e) => {
    if (ref && ref.current && e.which !== 3) {
      const { current } = ref;
      if (!current.contains(e.target)) {
        hideContextMenu();
      }
    }
  };

  useEffect(() => {
    const main = document.getElementById('main');
    if (main) main.addEventListener('scroll', onScroll);
    window.addEventListener('mousedown', onClick);
    window.addEventListener('touchstart', onClick);
    return () => {
      if (main) main.removeEventListener('scroll', onScroll);
      window.removeEventListener('mousedown', onClick);
      window.removeEventListener('touchstart', onClick);
    };
  }, []);

  useEffect(() => {
    setSubmenu(false);
  }, [context_menu]);

  if (!context_menu) return null;

  const style = {
    left: context_menu?.position_x,
    top: context_menu?.position_y,
  };
  const height = 200; // TODO: use jquery to detect height
  let className = `context-menu ${context_menu?.context}`;
  if (submenu) className += ' context-menu--submenu-expanded';
  if (context_menu?.closing) className += ' context-menu--closing';

  if (context_menu?.position_x > (window.innerWidth - 174)) {
    style.left = 'auto';
    style.right = 10;
  }

  if (context_menu?.position_y > (window.innerHeight - height)) {
    style.top = 'auto';
    style.bottom = 10;
  }

  return (
    <div className={className} style={style} ref={ref}>
      <div className="context-menu__section context-menu__section--items">
        {context_menu?.type !== 'custom' && <Title context_menu={context_menu} />}
        <ContextMenuItems
          context_menu={context_menu}
          onSubmenu={() => setSubmenu(true)}
        />
      </div>
      {submenu && (
        <PlaylistSubmenu
          context_menu={context_menu}
          allPlaylists={playlists}
          onClose={() => setSubmenu(false)}
          loading={loading_progress}
          providers={providers}
        />
      )}
      <div className="context-menu__background" onClick={hideContextMenu} />
    </div>
  );
};

const librarySelector = makeLibrarySelector('playlists', false);
const providersSelector = makeProvidersSelector('playlists');
const processProgressSelector = makeProcessProgressSelector(processKeys);

const mapStateToProps = (state) => ({
  providers: providersSelector(state),
  playlists: librarySelector(state, 'playlists'),
  loading_progress: processProgressSelector(state),
  context_menu: state.ui.context_menu,
  load_queue: state.ui.load_queue,
  processes: state.ui.processes,
  current_track: state.core.current_track,
  current_tracklist: state.core.current_tracklist,
  queue_metadata: state.core.queue_metadata,
  pinned: state.pusher.pinned,
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
