import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import { Grid } from '../../components/Grid';
import Loader from '../../components/Loader';
import Icon from '../../components/Icon';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { formatImages, encodeUri } from '../../util/format';
import { I18n, i18n } from '../../locale';
import { makeLoadingSelector } from '../../util/selectors';

class Browse extends React.Component {
  componentDidMount() {
    this.loadDirectory();
    this.props.uiActions.setWindowTitle(i18n('library.browse.title'));
  }

  loadDirectory = () => {
    const {
      mopidyActions: {
        getDirectory,
      },
    } = this.props;

    getDirectory(null);
  }

  render = () => {
    const {
      loading,
      directory,
    } = this.props;

    if (!directory) {
      if (loading) {
        return <Loader body loading />;
      }
      return null;
    }

    const grid_items = [];
    if (directory.subdirectories) {
      for (const subdirectory of directory.subdirectories) {
        switch (subdirectory.name) {
          case 'Dirble':
            subdirectory.icons = ['assets/backgrounds/browse-dirble.jpg'];
            break;

          case 'Files':
            subdirectory.icons = ['assets/backgrounds/browse-folders.jpg'];
            break;

          case 'Local media':
            subdirectory.icons = ['assets/backgrounds/browse-folders.jpg'];
            break;

          case 'Spotify':
          case 'Spotify Browse':
          case 'Spotify Web Browse':
            subdirectory.icons = ['assets/backgrounds/browse-spotify.jpg'];
            break;

          case 'Spotify Tunigo':
          case 'Tunigo':
            subdirectory.icons = ['assets/backgrounds/browse-tunigo.jpg'];
            break;

          case 'TuneIn':
            subdirectory.icons = ['assets/backgrounds/browse-tunein.jpg'];
            break;

          case 'SoundCloud':
            subdirectory.icons = ['assets/backgrounds/browse-soundcloud.jpg'];
            break;

          case 'Mixcloud':
            subdirectory.icons = ['assets/backgrounds/browse-mixcloud.jpg'];
            break;

          case 'iTunes Store: Podcasts':
            subdirectory.icons = ['assets/backgrounds/browse-itunes.jpg'];
            break;

          case 'SomaFM':
            subdirectory.icons = ['assets/backgrounds/browse-somafm.jpg'];
            break;
      
          case 'Jellyfin':
            subdirectory.icons = ['assets/backgrounds/browse-jellyfin.jpg'];
            break;
      
          case 'Podcasts':
            subdirectory.icons = ['assets/backgrounds/browse-podcasts.jpg'];
            break;

          case 'Tidal':
            subdirectory.icons = ['assets/backgrounds/browse-tidal.jpg'];
            break;

          case 'Google':
          case 'Google Play':
          case 'Google Play Music':
            subdirectory.icons = ['assets/backgrounds/browse-google.jpg'];
            break;

          case 'YouTube Music':
            subdirectory.icons = ['assets/backgrounds/browse-youtube.jpg'];
            break;

          case 'bandcamp':
          case 'Bandcamp':
            subdirectory.icons = ['assets/backgrounds/browse-default.jpg'];
            break;

          default:
            if (subdirectory.images?.medium) {
              subdirectory.icons = [subdirectory.images?.medium];
            } else {
              subdirectory.icons = ['assets/backgrounds/browse-default.jpg'];
            }
        }

        grid_items.push({
          name: subdirectory.name,
          link: `/library/browse/${encodeUri(subdirectory.uri)}/${encodeURIComponent(subdirectory.name)}`,
          icons: formatImages(subdirectory.icons),
        });
      }
    }

    return (
      <div className="view library-local-view">
        <Header>
          <Icon name="folder" type="material" />
          <I18n path="library.browse.title" />
        </Header>
        <section className="content-wrapper">
          <div className="grid grid--tiles">
            <Grid items={grid_items} sourceIcon={false} />
          </div>
        </section>
      </div>
    );
  }
}

const loadingSelector = makeLoadingSelector(['(.*)mopidy_library.browse(.*)']);
const mapStateToProps = (state) => {
  const {
    mopidy: {
      directory: _directory = {},
    },
    ui: {
      library_directory_view: view,
    },
  } = state;
  const directory = _directory && _directory.uri === null ? _directory : null;

  return {
    loading: loadingSelector(state),
    directory,
    view,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Browse);
