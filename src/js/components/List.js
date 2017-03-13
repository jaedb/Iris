
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { hashHistory, Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import Dater from './Dater'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class List extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e, uri){

		// make sure we haven't clicked a nested link (ie Artist name)
		if( e.target.tagName.toLowerCase() !== 'a' ){
			hashHistory.push( this.props.link_prefix + uri );
		}		
	}

	handleContextMenu(e,item){
		if (this.props.handleContextMenu){
			e.preventDefault()
			this.props.handleContextMenu(e,item)
		}
	}

	renderHeader(){
		if (!this.props.columns || this.props.noheader) return null

		return (
			<div className="list-item header cf">
				{
					this.props.columns.map( (col, col_index) => {
						var className = 'col '+col.name.replace('.','_')
						return <div className={className} key={col_index}>{ col.label ? col.label : col.name }</div>
					})
				}
			</div>
		)
	}

	renderValue( row, key_string ){
		var key = key_string.split('.')
		var value = row

		for (var i = 0; i < key.length; i++) {
			if (typeof(value[key[i]]) === 'undefined') {
				return <span>-</span>
			} else if ( typeof(value[key[i]]) === 'string' && value[key[i]].replace(' ','') == '') {
				return <span>-</span>
			} else {
				value = value[key[i]]
			}
		}

		if (key_string === 'added_at') return <span><Dater type="ago" data={value} /> ago</span>
		if (key_string === 'owner') return <Link to={global.baseURL+'user/'+ value.uri}>{value.id}</Link>
		if (key[0] === 'artists') return <ArtistSentence artists={value} />
		if (value === true) return <FontAwesome name="check" />
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
				{ this.renderHeader() }
				{
					this.props.rows.map( (row, row_index) => {

						var class_name = 'list-item'
						if( row.type ) class_name += ' '+row.type

						return (
							<div 
								onClick={e => this.handleClick(e, row.uri)} 
								onContextMenu={e => this.handleContextMenu(e,row)}
								className={class_name} 
								key={row_index}>
								{
									this.props.columns.map( (col, col_index) => {
										var className = 'col '+col.name.replace('.','_')
										return (
											<div className={className} key={col_index}>
												{ this.renderValue(row, col.name) }
											</div>
										)
									})
								}
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
