import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import { I18n } from '../locale';

class Dragger extends React.Component {
  constructor(props) {
    super(props);
    this.handleMouseMove = this.handleMouseMove.bind(this);
    this.handleMouseUp = this.handleMouseUp.bind(this);

    this.state = {
      active: false,
      position_x: 0,
      position_y: 0,
    };
  }

  componentDidMount() {
    window.addEventListener('mousemove', this.handleMouseMove, false);
    window.addEventListener('mouseup', this.handleMouseUp, false);
  }

  componentWillUnmount() {
    window.removeEventListener('mousemove', this.handleMouseMove, false);
    window.removeEventListener('mouseup', this.handleMouseUp, false);
  }

  handleMouseMove(e) {
    const { dragger, uiActions: { dragActive } } = this.props;
    const { target } = e;
    if (!dragger) return null;

    const threshold = 10;
    if (
      e.clientX > dragger.start_x + threshold
			|| e.clientX < dragger.start_x - threshold
			|| e.clientY > dragger.start_y + threshold
			|| e.clientY < dragger.start_y - threshold) {
      this.setState({
        position_x: e.clientX,
        position_y: e.clientY,
      });

      const dropzones = document.getElementsByClassName('dropzone');
      for (let i = 0; i < dropzones.length; i++) {
        dropzones[i].classList.remove('hover');
      }

      if (target.classList && target.classList.contains('dropzone') && !target.classList.contains('hover')) {
        target.className += ' hover';
      }

      // if not already, activate
      if (!dragger.active) dragActive();
    }
  }

  handleMouseUp(e) {
    const { dragger, uiActions: { dragEnd } } = this.props;
    if (!dragger) return null;
    dragEnd(e);
  }

  render() {
    const { dragger: { active, victims } = {} } = this.props;
    const { position_x, position_y } = this.state;

    if (!active) return null;

    return (
      <div
        className="dragger"
        style={{
          left: position_x,
          top: position_y,
        }}
      >
        <I18n path="dropzones.dragging_things" count={victims.length} />
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  dragger: state.ui.dragger,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Dragger);
