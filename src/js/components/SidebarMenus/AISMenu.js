import React, {Component} from 'react'
import { Link } from 'react-router'
import Icon from '../Icon'


class AISMenu extends Component {

  linkClassName = (link) => {
    if (this.props.location.pathname.startsWith('/'+link)){
      return 'active'
    } else {
      return null
    }
  }

  render = () => {
    if(this.props.enabled) {
    return <section>
      <title>AI-Speaker</title>
      <Link className={this.linkClassName('library/browse/ais:root:1')} to={global.baseURL+"library/browse/ais:root:1"}>
        <Icon name="ais" />
        Discover
      </Link>
      <Link className={this.linkClassName('library/playlists')} to={global.baseURL+"library/playlists"}>
        <Icon name="playlist" />
        Playlists
      </Link>
      <Link className={this.linkClassName('library/browse/ais:root:2')} to={global.baseURL+"library/browse/ais:root:2"}>
        <Icon name="ais-library" />
        My Library
      </Link>
      <Link className={this.linkClassName('library/browse/ais:root:3')} to={global.baseURL+"library/browse/ais:root:3"}>
        <Icon name="ais-family" />
        My Family
      </Link>
      <Link className={this.linkClassName('library/browse/ais:root:4')} to={global.baseURL+"library/browse/ais:root:4"}>
        <Icon name="ais-likes" />
        My Likes
      </Link>
      <Link className={this.linkClassName('library/browse/ais:root:5')} to={global.baseURL+"library/browse/ais:root:5"}>
        <Icon name="ais-pendrive" />
        Pendrive
      </Link>
    </section>
    } else {
      return null;
    }
  }
}

export default AISMenu