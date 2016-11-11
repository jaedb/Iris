
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

import FontAwesome from 'react-fontawesome'

export default class List extends React.Component{

	constructor(props) {
		super(props);
	}

	renderHeader(){
		if( !this.props.columns ) return null

		return (
			<div className="list-item header cf">
				{
					this.props.columns.map( (col, col_index) => {
						return <div className={'col w'+col.width} key={col_index}>{ col.name }</div>
					})
				}
			</div>
		)
	}

	renderValue( value ){
		if( typeof(value) === 'undefined' ) return <span>-</span>
		if( value === true ) return <FontAwesome name="check" />
		return <span>{value}</span>
	}

	render(){
		if( !this.props.rows ) return null

		return (
			<div className="list">
				{ this.renderHeader() }
				{
					this.props.rows.map( (row, row_index) => {
						return (
							<Link to={ this.props.link_prefix + encodeURIComponent(row.uri)} className="list-item cf" key={row_index}>
								{
									this.props.columns.map( (col, col_index) => {
										return (
											<div className={'col w'+col.width} key={col_index}>
												{ this.renderValue(row[col.name]) }
											</div>
										)
									})
								}
							</Link>
						)
					})
				}
			</div>
		);
	}
}