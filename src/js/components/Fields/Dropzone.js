import React from 'react';
import Icon from '../Icon';

export default class Dropzone extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      hover: false,
    };

    this.handleMouseOver = this.handleMouseOver.bind(this);
    this.handleMouseOut = this.handleMouseOut.bind(this);
  }

  componentDidMount() {
    window.addEventListener('mouseover', this.handleMouseOver, false);
    window.addEventListener('mouseout', this.handleMouseOut, false);
  }

  componentWillUnmount() {
    window.removeEventListener('mouseover', this.handleMouseOver, false);
    window.removeEventListener('mouseout', this.handleMouseOut, false);
  }

  handleMouseOver = () => {
    this.setState({ hover: true });
  }

  handleMouseOut = () => {
    this.setState({ hover: false });
  }

  render = () => {
    const {
      data,
      handleMouseUp,
    } = this.props;
    const { hover } = this.state;

    if (!data) return null;

    return (
      <div
        className={hover ? 'dropzone hover' : 'dropzone'}
        onMouseUp={handleMouseUp}
      >
        <Icon name={data.icon} />
        <span className="title">{ data.title }</span>
      </div>
    );
  }
}
