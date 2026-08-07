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

  static getDerivedStateFromProps(props, state) {
    if (props.loadKey && props.loadKey !== state.loadKey) {
      return {
        loadKey: props.loadKey,
        listening: true,
      };
    }
    return null;
  }

  handleScroll(e) {
    const { listening } = this.state;
    const { loadKey, loadMore } = this.props;
    const { scrollTop, scrollHeight, offsetHeight } = this.element || {};

    if (listening) {
      const window_height = Math.max(document.documentElement.clientHeight, window.innerHeight || 0);

      // At, or half a screen from bottom of the page
		  if (scrollTop > (scrollHeight - offsetHeight - (window_height / 2))) {
        // Immediately stop listening to avoid duplicating pagination requests
        this.setState(
          { listening: false },
          () => {
            console.info(`Loading more: ${loadKey}`);
            loadMore();
          },
        );
		  }
    }
  }

  render = () => {
    const { showLoader } = this.props;
    return (
      <Loader body lazy loading={showLoader} />
    );
  }
}
