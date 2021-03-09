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

class LibraryBrowse extends React.Component {
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
      mopidyActions,
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
            subdirectory.icons = ['/iris/assets/backgrounds/browse-dirble.jpg'];
            break;

          case 'Files':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-folders.jpg'];
            break;

          case 'Local media':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-folders.jpg'];
            break;

          case 'Spotify':
          case 'Spotify Browse':
          case 'Spotify Web Browse':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-spotify.jpg'];
            break;

          case 'Spotify Tunigo':
          case 'Tunigo':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-tunigo.jpg'];
            break;

          case 'TuneIn':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-tunein.jpg'];
            break;

          case 'SoundCloud':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-soundcloud.jpg'];
            break;

          case 'Mixcloud':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-mixcloud.jpg'];
            break;

          case 'iTunes Store: Podcasts':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-itunes.jpg'];
            break;

          case 'Soma FM':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-somafm.jpg'];
            break;

          case 'Tidal':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-tidal.jpg'];
            break;

          case 'Google':
          case 'Google Play':
          case 'Google Play Music':
            subdirectory.icons = ['/iris/assets/backgrounds/browse-google.jpg'];
            break;

          default:
            subdirectory.icons = ['/iris/assets/backgrounds/browse-default.jpg'];
        }

        grid_items.push({
          name: subdirectory.name,
          link: `/library/browse/${encodeURIComponent(subdirectory.name)}/${encodeUri(subdirectory.uri)}`,
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
            <Grid items={grid_items} />
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowse);
