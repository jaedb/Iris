
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { hashHistory } from 'react-router'

import ArtistSentence from './ArtistSentence'
import Dater from './Dater'
import URILink from './URILink'
import ContextMenuTrigger from './ContextMenuTrigger'
import Icon from './Icon'
import Popularity from './Popularity'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class List extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e, uri){

		// make sure we haven't clicked a nested link (ie Artist name)
		if (e.target.tagName.toLowerCase() !== 'a'){
			e.preventDefault();
			hashHistory.push((this.props.link_prefix ? this.props.link_prefix : '') + encodeURIComponent(uri));
		}
	}

	handleMouseDown(e, uri){

		// make sure we haven't clicked a nested link (ie Artist name)
		if (e.target.tagName.toLowerCase() !== 'a'){
			e.preventDefault();
			hashHistory.push((this.props.link_prefix ? this.props.link_prefix : '') + encodeURIComponent(uri));
		}
	}

	handleContextMenu(e,item){
		if (this.props.handleContextMenu){
			e.preventDefault();
			this.props.handleContextMenu(e,item);
		}
	}

	renderValue(row, key_string){
		var key = key_string.split('.');
		var value = row;

		for (var i = 0; i < key.length; i++){
			if (value[key[i]] === undefined){
				return null
			} else if (typeof(value[key[i]]) === 'string' && value[key[i]].replace(' ','') == ''){
				return null
			} else {
				value = value[key[i]]
			}
		}

		if (key_string === 'tracks_total') return <span>{value} tracks</span>
		if (key_string === 'followers') return <span>{value.toLocaleString()} followers</span>
		if (key_string === 'added_at') return <span><Dater type="ago" data={value} /> ago</span>
		if (key_string === 'owner') return <URILink type="user" uri={value.uri}>{value.id}</URILink>
		if (key_string === 'popularity') return <Popularity full popularity={value} />
		if (key[0] === 'artists') return <ArtistSentence artists={value} />
		if (value === true) return <Icon name="check" />
		if (typeof(value) === 'number') return <span>{value.toLocaleString()}</span>
		return <span>{value}</span>
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
					this.props.rows.map((row, row_index) => {

						var class_name = 'list__item'
						if (row.type ){
							class_name += ' list__item--'+row.type;
						}

						return (
							<div
								className={class_name} 
								key={row_index}
								onClick={e => this.handleClick(e, row.uri)}
								onContextMenu={e => this.handleContextMenu(e,row)}>

									<div className="list__item__actions">
										{
											(this.props.actions ? this.props.actions.map((action, index) => {
												return (
													<li className={'list__item__actions__item list__item__actions__item--'+action.replace('.','_')} key={index}>
														{this.renderValue(row, action)}
													</li>
												)
											}) : null)
										}

										{this.props.nocontext ? null : <ContextMenuTrigger className="list__item__actions__item list__item__actions__item--context-menu-trigger subtle" key="context" onTrigger={e => this.handleContextMenu(e, row)} />}

									</div>

									<div className="list__item__content">

										<div className="list__item__name">
											{this.renderValue(row, 'name')}
										</div>

										<ul className="list__item__details">
											{
												this.props.details.map((detail, index) => {
													var value = this.renderValue(row, detail);

													if (!value){
														return null;
													}

													return (
														<li className={'list__item__details__item list__item__details__item--'+detail.replace('.','_')} key={index}>
															{value}
														</li>
													)
												})
											}
										</ul>
									</div>
							</div>
						)
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
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(List)
