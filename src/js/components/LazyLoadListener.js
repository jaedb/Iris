
import React from 'react';
import { throttle } from '../util/helpers';
import Loader from './Loader';

export default class LazyLoadListener extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      listening: (!!this.props.loadKey),
      loadKey: this.props.loadKey,
    };

    this.handleScroll = throttle(this.handleScroll.bind(this), 50);
  }

  componentDidMount() {
    this.element = document.getElementById('main');
    this.element.addEventListener('scroll', this.handleScroll, false);
  }

  componentWillUnmount() {
    this.element.removeEventListener('scroll', this.handleScroll, false);
  }

  componentWillReceiveProps(nextProps) {
    if (nextProps.loadKey && nextProps.loadKey !== this.state.loadKey) {
      this.setState({
        loadKey: nextProps.loadKey,
        listening: true,
      });
    }
  }

  handleScroll(e) {
    if (this.state.listening) {
      const window_height = Math.max(document.documentElement.clientHeight, window.innerHeight || 0);

      // At, or half a screen from bottom of the page
		    if (this.element.scrollTop > (this.element.scrollHeight - this.element.offsetHeight - (window_height / 2))) {
        // Immediately stop listening to avoid duplicating pagination requests
        this.setState(
          { listening: false },
          () => {
            console.info(`Loading more: ${this.props.loadKey}`);
            this.props.loadMore();
          },
        );
		    }
    }
  }

  render() {
    return (
      <Loader body lazy loading={this.props.showLoader} />
    );
  }
}
