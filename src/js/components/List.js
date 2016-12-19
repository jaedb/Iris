
import React, { PropTypes } from 'react'
import { Link, browserHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'

import * as helpers from '../helpers'

export default class List extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e, uri){

		// make sure we haven't clicked a nested link (ie Artist name)
		if( e.target.tagName.toLowerCase() !== 'a' ){
			browserHistory.push( this.props.link_prefix + encodeURIComponent(uri) );
		}		
	}

	renderHeader(){
		if (!this.props.columns || this.props.noheader) return null

		return (
			<div className="list-item header cf">
				{
					this.props.columns.map( (col, col_index) => {
						return <div className={'col w'+col.width} key={col_index}>{ col.label ? col.label : col.name }</div>
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

		if (key_string === 'owner') return <Link to={ '/user/'+ value.uri }>{value.id}</Link>
		if (key[0] === 'artists') return <ArtistSentence artists={value} />
		if (value === true) return <FontAwesome name="check" />
		if (typeof(value) === 'number') return <span>{value.toLocaleString()}</span>
		return <span>{value}</span>
	}

	render(){
		if (!this.props.rows) return null

		return (
			<div className="list">
				{ this.renderHeader() }
				{
					this.props.rows.map( (row, row_index) => {

						var className = 'list-item'
						if( row.type ) className += ' '+row.type

						return (
							<div onClick={ e => this.handleClick(e, row.uri)} className={className} key={row_index}>
								{
									this.props.columns.map( (col, col_index) => {
										return (
											<div className={'col w'+col.width} key={col_index}>
												{ this.renderValue(row, col.name) }
											</div>
										)
									})
								}
								{ this.props.show_source_icon ? <FontAwesome className="source" name={helpers.sourceIcon(row.uri)} /> : null }
							</div>
						)
					})
				}
			</div>
		);
	}
}