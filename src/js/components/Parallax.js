
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Parallax extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			scrollTop: 0,
			windowWidth: 0,
			windowHeight: 0,
			canvas: {
				width: 0,
				height: 0
			},
			original: {},
			image: {},
			imageObject: {}
		}
	}

	componentDidMount(){
		var url = helpers.SizedImages( this.props.images ).huge;
		this.loadImage( url )
			.then(
				response => {
					this.setState({ image: response })
					this.updateCanvas( response )
				}
			)

		$(window).resize(() => this.updateCanvas( this.state.image ));
	}

	loadImage( url ){		
		return new Promise( (resolve, reject) => {

			var imageObject = new Image();
			imageObject.src = url;

			imageObject.onload = function(){
				var image = {
					width: imageObject.naturalWidth,
					height: imageObject.naturalHeight,
					original_width: imageObject.naturalWidth,
					original_height: imageObject.naturalHeight,
					object: imageObject
				}
				resolve( image )
			}
		})
	}

	updateCanvas( image ){
		var canvasWidth = $('.parallax').outerWidth();
		var canvasHeight = $('.parallax').outerHeight();
		if( this.state.canvas.width != canvasWidth || this.state.canvas.height != canvasHeight ){
			this.setState({
				canvas: {
					width: canvasWidth,
					height: canvasHeight
				}
			})
		}
		this.renderCanvas( image );
	}

	renderCanvas( image ){

		let self = this;
		var canvasDOM = document.getElementById('parallax-canvas');
		var context = canvasDOM.getContext('2d');
		
		// zoom image to fill canvas, widthwise
		if( image.width < this.state.canvas.width || image.width > this.state.canvas.width ){
			var scale = this.state.canvas.width / image.width;
			image.width = image.width * scale;
			image.height = image.height * scale;
		}
		
		// now check for fill heightwise, and zoom in if necessary
		if( image.height < this.state.canvas.height ){
			var scale = this.state.canvas.height / image.height;
			image.width = image.width * scale;
			image.height = image.height * scale;
		}
		
		// figure out where we want the image to be, based on scroll position
		var percent = Math.round( self.state.scrollTop / this.state.canvas.height * 100 );
		var position = Math.round( (this.state.canvas.height / 2) * (percent/100) ) - 100;
		
		image.x = ( this.state.canvas.width / 2 ) - ( image.width / 2 );
		image.y = ( ( this.state.canvas.height / 2 ) - ( image.height / 2 ) ) + ( ( percent / 100 ) * 100);

		// actually draw the image on the canvas
		context.drawImage(image.object, image.x, image.y, image.width, image.height);

		// now update our component
		self.setState({ image: image });
		
		// poll for scroll changes
		/*
		var animateInterval = $interval(
			function(){	
				window.requestAnimationFrame(function( event ){
					
					var bannerPanel = $(document).find('.intro');
					
					// if we've scrolled or resized
					if(
						state.scrollTop != $(document).scrollTop() ||
						state.windowWidth != $(window).width() || 
						state.windowHeight != $(window).height() ){
							
							// update our state
							state.scrollTop = $(document).scrollTop();
							state.windowWidth = $(window).width();
							state.windowHeight = $(window).height();
							
							var bannerHeight = bannerPanel.outerHeight();

							// and if we're within the bounds of our document
							// this helps prevent us animating when the objects in question are off-screen
							if( state.scrollTop < bannerHeight ){								
								positionArtistBackground( image );
							}
						}
				});
			},
			10
		);*/

	}

	render(){
		return (
			<div className="parallax">
				<canvas id="parallax-canvas" width={this.state.canvas.width} height={this.state.canvas.height}></canvas>
			</div>
		);
	}
}