import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';
import ListItem from './ListItem';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as discogsActions from '../services/discogs/actions';

class List extends React.Component {
  handleContextMenu(e, item) {
    const { handleContextMenu } = this.props;

    if (handleContextMenu) {
      e.preventDefault();
      handleContextMenu(e, item);
    }
  }

  render = () => {
    const {
      rows,
      className,
    } = this.props;

    if (!rows) return null;

    return (
      <div className={`list ${className}`}>
        {
					rows.map((item, index) => (
            <ListItem
              key={index}
              item={item}
              lastfmActions={this.props.lastfmActions}
              discogsActions={this.props.discogsActions}
              history={this.props.history}
              link_prefix={this.props.link_prefix}
              handleContextMenu={(e) => this.handleContextMenu(e, item)}
              thumbnail={this.props.thumbnail}
              details={this.props.details}
              nocontext={this.props.nocontext}
            />
					))
				}
      </div>
    );
  }
}

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  discogsActions: bindActionCreators(discogsActions, dispatch),
});

export default withRouter(connect(mapDispatchToProps)(List));
