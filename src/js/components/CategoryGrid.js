


import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class CategoryGrid extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		if (!this.props.categories ) return null

		var className = "grid grid--categories";
		if (this.props.className) className += ' '+this.props.className;
		if (this.props.mini) className += ' grid--mini';

		return (
			<div className={className}>
				{
					this.props.categories.map(category => {
						return (
							<GridItem 
								key={category.id}
								type="category"
								item={category}
								link={global.baseURL+'discover/categories/'+encodeURIComponent(category.id)}
							/>
						)
					})
				}
			</div>
		);
	}
}

