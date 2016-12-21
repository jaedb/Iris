
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Header from '../../components/Header'
import GridItem from '../../components/GridItem'

export default class LibraryLocal extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){

		var grid_items = [
			{
				name: 'Artists',
				link: global.baseURL+'library/local/artists',
				icons: ['assets/backgrounds/category-artists.jpg']
			},
			{
				name: 'Albums',
				link: global.baseURL+'library/local/albums',
				icons: ['assets/backgrounds/category-albums.jpg']
			},
			{
				name: 'Folders',
				link: global.baseURL+'library/local/directory/local:directory',
				icons: ['assets/backgrounds/category-folders.jpg']
			}
		]

		return (
			<div className="view library-local-view">
				<Header icon="folder" title="Local" />
				<section className="grid-wrapper">
					<div className="grid category-grid">				
						{
							grid_items.map(
								(item, index) => {
									return <GridItem item={item} key={index} link={item.link} />
								}
							)
						}
					</div>
				</section>
			</div>
		);
	}
}