
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import GridItem from '../../components/GridItem';
import Icon from '../../components/Icon';
import ErrorBoundary from '../../components/ErrorBoundary';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { formatImages } from '../../util/format';

class LibraryBrowse extends React.Component {
  componentDidMount() {
    this.loadDirectory();
    this.props.uiActions.setWindowTitle('Browse');
  }

  componentDidUpdate = ({ mopidy_connected: prev_mopidy_connected}) => {
    const { mopidy_connected } = this.props;
    if (!prev_mopidy_connected && mopidy_connected) this.loadDirectory();
  }

  loadDirectory(props = this.props) {
    if (props.mopidy_connected) {
      this.props.mopidyActions.getDirectory(null);
    }
  }

  render() {
    const grid_items = [];
    if (this.props.directory) {
      for (const subdirectory of this.props.directory.subdirectories) {
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
          link: `/library/browse/${encodeURIComponent(subdirectory.uri)}`,
          icons: formatImages(subdirectory.icons),
        });
      }
    }

    return (
      <div className="view library-local-view">
        <Header>
          <Icon name="folder" type="material" />
					Browse
        </Header>
        <section className="content-wrapper">
          <div className="grid grid--tiles">
            <ErrorBoundary>
              {
								grid_items.map(
								  (item, index) => (
                    <GridItem
                      item={item}
                      key={index}
                      link={item.link}
                      mopidyActions={this.props.mopidyActions}
                      type="browse"
                    />
								  ),
								)
							}
            </ErrorBoundary>
          </div>
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  load_queue: state.ui.load_queue,
  mopidy_connected: state.mopidy.connected,
  directory: state.mopidy.directory,
  view: state.ui.library_directory_view,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowse);
