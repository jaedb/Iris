
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import Link from '../Link';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';

class FollowButton extends React.Component {
  constructor(props) {
    super(props);
  }

  remove() {
    this.props.spotifyActions.following(this.props.uri, 'DELETE');
  }

  add() {
    this.props.spotifyActions.following(this.props.uri, 'PUT');
  }

  render() {
    if (!this.props.uri) {
      return false;
    }

    let className = 'button';
    if (this.props.className) {
      className += ` ${this.props.className}`;
    }

    if (!this.props.spotify_authorized) {
      return <button className={`${className} button--disabled`} onClick={(e) => this.props.uiActions.createNotification({ content: 'You must authorize Spotify first', type: 'warning' })}>{this.props.addText}</button>;
    } if (this.props.is_following === true) {
      return <button className={`${className} button--destructive`} onClick={(e) => this.remove()}>{this.props.removeText}</button>;
    }
    return <button className={`${className} button--default`} onClick={(e) => this.add()}>{this.props.addText}</button>;
  }
}

const mapStateToProps = (state, ownProps) => ({
  load_queue: state.ui.load_queue,
  spotify_authorized: state.spotify.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton);
