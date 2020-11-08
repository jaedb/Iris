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
      lastfmActions,
      discogsActions,
      history,
      link_prefix,
      thumbnail,
      details,
      nocontext,
    } = this.props;

    if (!rows) return null;

    return (
      <div className={`list ${className}`}>
        {
					rows.map((item, index) => (
            <ListItem
              key={index}
              item={item}
              lastfmActions={lastfmActions}
              discogsActions={discogsActions}
              history={history}
              link_prefix={link_prefix}
              handleContextMenu={(e) => this.handleContextMenu(e, item)}
              thumbnail={thumbnail}
              details={details}
              nocontext={nocontext}
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
