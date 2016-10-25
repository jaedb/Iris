
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Parallax extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			scrollTop: 0,
			windowWidth: 0,
			windowHeight: 0,
			source: {},
			canvas: {
				width: 0,
				height: 0
			},
			image: {}
		}
	}

	componentDidMount(){

		this.loadImage( helpers.SizedImages( this.props.images ).huge )
			.then(
				response => this.setState({ image: response, source: {} })
			)

		this.updateCanvas()
		$(window).resize(() => this.updateCanvas());
	}

	loadImage( url ){		
		return new Promise( (resolve, reject) => {

			var imageObject = new Image();
			imageObject.src = url;

			imageObject.onload = function(){
				resolve( imageObject )
			}
		})
	}

	updateCanvas(){
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
		console.log( this.state );
	}

	update(){

		let self = this;
			
		var parallax = $('.parallax');
		var canvasDOM = document.getElementById('parallax-canvas');
		var context = canvasDOM.getContext('2d');
		

		var image = {};
		Object.assign(image, self.state.source);

		// set our canvas dimensions (if necessary)
		var canvasWidth = $('#parallax-canvas').outerWidth();
		var canvasHeight = $('#parallax-canvas').outerHeight();
		if( context.canvas.width != canvasWidth || context.canvas.height != canvasHeight ){
			context.canvas.width = canvasWidth;
			context.canvas.height = canvasHeight;
		}
		
		// zoom image to fill canvas, widthwise
		if( image.width < canvasWidth || image.width > canvasWidth ){
			var scale = canvasWidth / image.width;
			image.width = image.width * scale;
			image.height = image.height * scale;
		}
		
		// now check for fill heightwise, and zoom in if necessary
		if( image.height < canvasHeight ){
			var scale = canvasHeight / image.height;
			image.width = image.width * scale;
			image.height = image.height * scale;
		}
		
		// figure out where we want the image to be, based on scroll position
		var percent = Math.round( self.state.scrollTop / canvasHeight * 100 );
		var position = Math.round( (canvasHeight / 2) * (percent/100) ) - 100;
		
		image.x = ( canvasWidth / 2 ) - ( image.width / 2 );
		image.y = ( ( canvasHeight / 2 ) - ( image.height / 2 ) ) + ( ( percent / 100 ) * 100);

		// actually draw the image on the canvas
		context.drawImage(imageObject, image.x, image.y, image.width, image.height);

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