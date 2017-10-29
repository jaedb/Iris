
import { hashHistory } from 'react-router'

import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class CategoryGrid extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		if (!this.props.categories ) return null

		var className = "grid category-grid"
		if (this.props.className ) className += ' '+this.props.className
		return (
			<div className={className}>
				{
					this.props.categories.map(category => {
						return (
							<GridItem 
								key={category.id}
								type="category"
								item={category}
								onClick={e => {hashHistory.push(global.baseURL+'discover/categories/'+encodeURIComponent(category.id))}}
							/>
						)
					})
				}
			</div>
		);
	}
}

