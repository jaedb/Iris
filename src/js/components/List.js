
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';
import { withRouter } from 'react-router';

import ListItem from './ListItem';

import * as helpers from '../helpers';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as discogsActions from '../services/discogs/actions';

class List extends React.Component{

	constructor(props){
		super(props);
	}

	handleContextMenu(e,item){
		if (this.props.handleContextMenu){
			e.preventDefault();
			this.props.handleContextMenu(e,item);
		}
	}

	render(){
		if (!this.props.rows) return null

		var className = 'list'
		if (this.props.className){
			className += ' '+this.props.className
		}

		return (
			<div className={className}>
				{
					this.props.rows.map((item, index) => {
						return (
							<ListItem
								key={index}
								item={item}
								lastfmActions={this.props.lastfmActions}
								discogsActions={this.props.discogsActions}
								history={this.props.history}
								link_prefix={this.props.link_prefix}
								handleContextMenu={e => this.handleContextMenu(e, item)}
								thumbnail={this.props.thumbnail}
								details={this.props.details}
								nocontext={this.props.nocontext}
							/>
						);
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		discogsActions: bindActionCreators(discogsActions, dispatch),
	}
}

export default withRouter(connect(mapStateToProps, mapDispatchToProps)(List));
