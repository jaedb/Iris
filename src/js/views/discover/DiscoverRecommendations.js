import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { compact } from 'lodash';
import InputRange from 'react-input-range';
import { Grid } from '../../components/Grid';
import TrackList from '../../components/TrackList';
import Thumbnail from '../../components/Thumbnail';
import Parallax from '../../components/Parallax';
import DropdownField from '../../components/Fields/DropdownField';
import AddSeedField from '../../components/Fields/AddSeedField';
import URILink from '../../components/URILink';
import ContextMenuTrigger from '../../components/ContextMenuTrigger';
import RelatedArtists from '../../components/RelatedArtists';
import Icon from '../../components/Icon';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import * as coreActions from '../../services/core/actions';
import {
  isLoading,
  getFromUri,
  uriType,
  titleCase,
} from '../../util/helpers';
import { arrayOf, indexToArray } from '../../util/arrays';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';

class Discover extends React.Component {
  constructor(props) {
    super(props);

    this._autocomplete_timer = false;

    this.state = {
      add_seed: '',
      adding_seed: false,
      seeds: [],
      tunabilities: {
        acousticness: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        danceability: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        energy: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        instrumentalness: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        key: {
          enabled: false,
          min: 0,
          max: 11,
          value: {
            min: 3,
            max: 8,
          },
        },
        liveness: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        loudness: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        popularity: {
          enabled: false,
          min: 0,
          max: 100,
          value: {
            min: 0,
            max: 100,
          },
        },
        speechiness: {
          enabled: false,
          convert_to_decimal: true,
          description: 'The presence of spoken words in a track',
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        tempo: {
          enabled: false,
          convert_to_decimal: true,
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
        valence: {
          enabled: false,
          convert_to_decimal: true,
          description: 'The musical positiveness conveyed by a track',
          min: 0,
          max: 100,
          value: {
            min: 25,
            max: 75,
          },
        },
      },
    };
  }

  componentDidMount() {
    const {
      uiActions: {
        setWindowTitle,
      },
      match: {
        params: {
          seeds,
        },
      },
    } = this.props;

    setWindowTitle(i18n('discover.recommendations.title'));

    if (seeds) {
      this.handleURLSeeds();
    }
  }

  componentDidUpdate = ({
    match: {
      params: {
        seeds: prevSeeds,
      },
    },
  }) => {
    const {
      match: {
        params: {
          seeds,
        },
      },
    } = this.props;
    if (prevSeeds !== seeds) this.handleURLSeeds();
  }

  handleContextMenu = (e) => {
    const {
      recommendations: {
        tracks_uris,
      },
      uiActions: {
        showContextMenu,
      },
      items: itemsProp,
    } = this.props;

    const tracks = indexToArray(itemsProp, tracks_uris);

    showContextMenu({
      e,
      context: 'track',
      items: tracks,
      uris: arrayOf('uri', tracks),
      tracklist_uri: this.uri(),
    });
  }

  handleURLSeeds = () => {
    const {
      coreActions: {
        loadUris,
      },
      match: {
        params: {
          seeds: seedsProp,
        },
      },
    } = this.props;

    // Rejoin if we've had to uri-encode these as strings
    // We'd need to do this if our URL has been encoded so the whole URL can become
    // it's own URI (eg iris:discover:spotify_artist_1234) where we can't use ":"
    const seeds = seedsProp.split('_').join(':').split(',');
    loadUris(seeds);

    this.setState(
      { seeds },
      () => this.getRecommendations(),
    );
  }

  uri = () => {
    const {
      seeds,
    } = this.state;

    let uri = 'iris:discover';
    if (seeds) {
      uri += ':';
      for (let i = 0; i < seeds.length; i++) {
        if (i > 0) {
          uri += ',';
        }
        uri += seeds[i].split(':').join('_');
      }
    }

    return uri;
  }

  getRecommendations = () => {
    const {
      seeds,
      tunabilities,
    } = this.state;
    const {
      spotifyActions: {
        getRecommendations: doGetRecommendations,
      },
    } = this.props;

    if (seeds.length > 0) {
      const digested_tunabilities = {};
      for (const key in tunabilities) {
        if (tunabilities.hasOwnProperty(key) && tunabilities[key].enabled) {
          const tunability = tunabilities[key];

          let { max } = tunability.value;
          let { min } = tunability.value;

          if (tunability.convert_to_decimal) {
            max /= 100;
            min /= 100;
          }

          digested_tunabilities[`${key}_max`] = max.toString();
          digested_tunabilities[`${key}_min`] = min.toString();
        }
      }
      doGetRecommendations(seeds, 50, digested_tunabilities);
    }
  }

  playTracks = () => {
    const {
      mopidyActions: {
        playURIs,
      },
      recommendations: {
        tracks_uris,
      },
    } = this.props;

    playURIs(tracks_uris, this.uri());
  }

  removeSeed = (index) => {
    const { seeds } = this.state;
    seeds.splice(index, 1);
    this.setState({ seeds });
  }

  handleSelect = (e, uri) => {
    const { seeds } = this.state;
    seeds.push(uri);
    this.setState({ seeds });
  }

  renderSeeds = () => {
    const {
      items,
    } = this.props;
    const {
      seeds,
    } = this.state;
    const seeds_objects = [];

    if (seeds.length > 0) {
      for (let i = 0; i < seeds.length; i++) {
        const uri = seeds[i];

        switch (uriType(uri)) {
          case 'genre':
            const name = getFromUri('genreid', uri);
            seeds_objects.push({
              name: (name.charAt(0).toUpperCase() + name.slice(1)).replace('-', ' '),
              uri,
            });
            break;
          default:
            if (typeof (items[uri]) !== 'undefined') {
              seeds_objects.push(items[uri]);
            } else {
              seeds_objects.push({
                name: 'Loading...',
                uri,
              });
            }
            break;
        }
      }
    }

    return (
      <div className="seeds">
        {
          seeds_objects.map((seed, index) => {
            const type = uriType(seed.uri);
            let images = null;
            if (seed.images) {
              if (type === 'artist') {
                if (seed.images.length > 0) {
                  images = seed.images[0];
                }
              } else {
                images = seed.images;
              }
            }

					  return (
  <div className={`seed${seed.images ? ' has-thumbnail' : ''}`} key={seed.uri}>
    {images && (
    <URILink className="thumbnail-wrapper" type={type} uri={seed.uri}>
      <Thumbnail images={images} circle={seed.type === 'artist'} size="small" />
    </URILink>
    )}
    <div className="seed__details">
      <div className="seed__label">
        <span className="seed__label__text">{titleCase(type)}</span>
        <Icon name="close" className="seed__label__remove" onClick={() => this.removeSeed(index)} />
      </div>
      <div className="seed__label__name">{seed.name}</div>
    </div>
  </div>
					  );
          })
				}
      </div>
    );
  }

  setTunability = (name, value) => {
    const { tunabilities } = this.state;
    tunabilities[name].value = value;
    this.setState({ tunabilities });
  }

  toggleTunability = (name) => {
    const { tunabilities } = this.state;
    tunabilities[name].enabled = !tunabilities[name].enabled;
    this.setState({ tunabilities });
  }

  renderTunabilities = () => {
    const addable_tunabilities = [];
    const enabled_tunabilities = [];
    for (const key in this.state.tunabilities) {
      if (this.state.tunabilities.hasOwnProperty(key)) {
        const tunability = {

          ...this.state.tunabilities[key],
          name: key,
        };

        if (tunability.enabled) {
          enabled_tunabilities.push(tunability);
        } else {
          addable_tunabilities.push({
            label: titleCase(tunability.name),
            value: tunability.name,
          });
        }
      }
    }

    return (
      <div className="tunabilities">
        {enabled_tunabilities.map((tunability) => (
          <div className="field tunability range" key={tunability.name}>
            <div className="tunability__label">
              {titleCase(tunability.name)}
              <span className="remove" onClick={() => this.toggleTunability(tunability.name)}>
                <Icon name="close" />
              </span>
            </div>
            <div className="tunability__input">
              <InputRange
                disabled={!tunability.enabled}
                minValue={tunability.min}
                maxValue={tunability.max}
                value={tunability.value}
                onChange={(value) => this.setTunability(tunability.name, value)}
              />
            </div>
          </div>
        ))}
      </div>
    );
  }

  renderResults = () => {
    const {
      recommendations: {
        tracks_uris,
        albums_uris,
        artists_uris,
      } = {},
      items: itemsProp,
    } = this.props;

    if (!albums_uris === undefined || artists_uris === undefined) {
      return <div className="content-wrapper recommendations-results" />;
    }

    const tracks = compact(tracks_uris.map((uri) => itemsProp[uri]));
    const artists = compact(artists_uris.map((uri) => itemsProp[uri]));
    const albums = compact(albums_uris.map((uri) => itemsProp[uri]));

    // Complete records not yet in our index
    if (tracks.length <= 0 && artists.length <= 0 && albums.length <= 0) {
      return null;
    }

    return (
      <div className="content-wrapper recommendations-results">

        <section className="col col--w70 tracks">
          <h4>
            <I18n path="discover.recommendations.tracks" />
            <div className="actions-wrapper">
              <ContextMenuTrigger onTrigger={this.handleContextMenu} />
              <Button
                type="primary"
                onClick={this.playTracks}
                tracking={{ category: 'DiscoverRecommendations', action: 'Play' }}
              >
                <I18n path="actions.play_all" />
              </Button>
            </div>
          </h4>
          <TrackList className="discover-track-list" uri={this.uri()} tracks={tracks} />
        </section>

        <div className="col col--w5" />

        <div className="col col--w25 others">
          <section>
            <h4><I18n path="discover.recommendations.artists" /></h4>
            <RelatedArtists artists={artists} uiActions={uiActions} />
          </section>
          <br />
          <br />
          <section>
            <h4><I18n path="discover.recommendations.albums" /></h4>
            <Grid items={albums} mini />
          </section>
        </div>

      </div>
    );
  }

  render = () => {
    const {
      load_queue,
      theme,
    } = this.props;
    const {
      tunabilities,
      seeds,
    } = this.state;

    const is_loading = isLoading(load_queue, ['spotify_recommendations']);
    const addable_tunabilities = [];
    for (const key in this.state.tunabilities) {
      if (tunabilities.hasOwnProperty(key)) {
        const tunability = {
          ...tunabilities[key],
          name: key,
        };

        if (!tunability.enabled) {
          addable_tunabilities.push({
            label: titleCase(tunability.name),
            value: tunability.name,
          });
        }
      }
    }

    return (
      <div className="view discover-view preserve-3d">
        <div className="intro preserve-3d">
          {theme === 'dark' && <Parallax image="/iris/assets/backgrounds/discover.jpg" />}
          <div className="intro__liner">
            <h1><I18n path="discover.recommendations.body_title" /></h1>
            <h2><I18n path="discover.recommendations.body_subtitle" /></h2>
            <div className="intro__parameters">
              {this.renderSeeds()}
              {this.renderTunabilities()}
              {seeds.length > 5 && (
                <p className="message error">
                  <I18n path="discover.recommendations.too_many_seeds" />
                </p>
              )}
            </div>
            <div className="intro__actions">
              <AddSeedField onSelect={(e, uri) => this.handleSelect(e, uri)} />
              <DropdownField
                className="add-properties"
                name="Properties"
                options={addable_tunabilities}
                no_status_icon
                button="default"
                handleChange={this.toggleTunability}
              />
              <div className="intro__actions__separator" />
              <Button
                className="submit"
                type="primary"
                size="large"
                working={is_loading}
                onClick={this.getRecommendations}
                tracking={{ category: 'DiscoverRecommendations', action: 'Submit' }}
              >
                <Icon name="explore" />
                <I18n path="discover.recommendations.find_recommendations" afterContent>
                  {' '}
                </I18n>
              </Button>
            </div>
          </div>

        </div>

        {this.renderResults()}

      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  theme: state.ui.theme,
  items: state.core.items,
  genres: (state.core.genres ? state.core.genres : []),
  authorized: state.spotify.authorization,
  load_queue: state.ui.load_queue,
  quick_search_results: (state.spotify.quick_search_results ? state.spotify.quick_search_results : { artists: [], tracks: [] }),
  recommendations: (state.spotify.recommendations ? state.spotify.recommendations : {}),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Discover);
