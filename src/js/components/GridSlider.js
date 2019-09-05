
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import Link from './Link';

import ArtistSentence from './ArtistSentence';
import Thumbnail from './Thumbnail';

import * as uiActions from '../services/ui/actions';

class GridSlider extends React.Component {
  constructor(props) {
    super(props);

    this._pagelimit = 3;

    this.state = {
      page: 0,
    };
  }

  handleClick(e, link) {
    if (e.target.tagName.toLowerCase() !== 'a') {
      this.props.history.push(link);
    }
  }

  handleContextMenu(e, item) {
    e.preventDefault();
    const data = {
      uris: [item.uri],
      item,
    };
    this.props.uiActions.showContextMenu(e, data, 'album', 'click');
  }

  next() {
    if (this.state.page >= this._pagelimit) return false;
    this.setState({ page: this.state.page + 1 });
  }

  previous() {
    if (this.state.page <= 0) return false;
    this.setState({ page: this.state.page - 1 });
  }

  render() {
    if (this.props.tracks) {
      let className = 'grid-slider-wrapper';
      if (this.props.className) className += ` ${this.props.className}`;

      const style = {
        left: `-${this.state.page * 100}%`,
      };

      return (
        <div className={className}>
          { this.props.title ? this.props.title : null }
          <div className="controls">
            <Icon type="fontawesome" name="chevron-left" disabled={this.state.page <= 0} onClick={() => this.previous()} />
            <Icon type="fontawesome" name="chevron-right" disabled={this.state.page >= this._pagelimit} onClick={() => this.next()} />
          </div>
          <div className="grid-slider">
            <div className="grid artist-grid liner" style={style}>
              {
								this.props.tracks.map(
								  (track, index) => {
								    const album = { ...track.album, artists: track.artists };
								    return (
  <div
    className="grid-item"
    key={index}
    onClick={(e) => this.handleClick(e, `/album/${album.uri}`)}
    onContextMenu={(e) => this.handleContextMenu(e, album)}
  >
    <Thumbnail size="medium" images={album.images} />
    <div className="name">{ album.name }</div>
    <div className="secondary">
      { album.artists ? <ArtistSentence artists={album.artists} /> : <span>-</span> }
    </div>
  </div>
								    );
								  },
								)
							}
            </div>
          </div>
        </div>
      );
    }
    return null;
  }
}

const mapStateToProps = (state, ownProps) => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(GridSlider);
