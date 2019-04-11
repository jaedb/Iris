
import React, { memo } from 'react'
import GridItem from './GridItem'

export default memo((props) => {
	
	if (!props.categories){
		return null;
	}

	let className = "grid grid--tiles";
	if (props.className) className += ' '+props.className;
	if (props.mini) className += ' grid--mini';

	return (
		<div className={className}>
			{
				props.categories.map(category => {
					return (
						<GridItem 
							key={category.id}
							type="category"
							item={category}
							link={'/discover/categories/'+encodeURIComponent(category.id)}
						/>
					)
				})
			}
		</div>
	);
});

