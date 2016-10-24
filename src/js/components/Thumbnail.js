
import React, { PropTypes } from 'react'

export default class Thumbnail extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			small: false,
			medium: false,
			large: false,
			huge: false
		}
	}

	componentDidMount(){
		this.mapImageSizes();
	}

	componentWillReceiveProps( nextProps ){
		this.mapImageSizes();
	}

	mapImageSizes(){
		var images = this.props.images;
		for( var i = 0; i < images.length; i++ ){
			if( images[i].height > 800 ){
				this.setState({ huge: images[i].url });
			}else if( images[i].height > 600 ){
				this.setState({ large: images[i].url });
			}else if( images[i].height > 280 ){
				this.setState({ medium: images[i].url });
			}else{
				this.setState({ small: images[i].url });
			}
		}
	}

	render(){
		return (
			<img className={ 'thumbnail '+ this.props.size } src={ this.state[this.props.size] } />
		);
	}
}