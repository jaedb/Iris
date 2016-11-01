
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

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

	render(){
		return (
			<div className="list">
				{ this.renderHeader() }
				{
					this.props.rows.map( (row, row_index) => {
						return (
							<Link to={ this.props.link_prefix + encodeURIComponent(row.uri)} className="list-item cf" key={row_index}>
								{
									this.props.columns.map( (col, col_index) => {
										return <div className={'col w'+col.width} key={col_index}>{ row[col.name] }</div>
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