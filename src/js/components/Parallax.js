
import React, { PropTypes } from 'react'

export default class Parallax extends React.Component{

	constructor(props) {
		super(props);

		this._loading = false
		this._loaded = false

		this.state = {
			scrollTop: 0,
			windowWidth: 0,
			windowHeight: 0,
			canvas: {
				width: 0,
				height: 0
			},
			image: {},
			loading: false,
			url: false
		}

		// we need to manually bind this as eventListener doesn't work with anonymous functions
		this.handleResize = this.handleResize.bind(this);
		this.handleScroll = this.handleScroll.bind(this);
	}

	componentDidMount(){
        this._mounted = true
        window.addEventListener("resize", this.handleResize);
        window.addEventListener("scroll", this.handleScroll);

        if( this.props.image ){
			this._loading = true
			this.setState({ url: this.props.image, image: false, loading: true })
			this.loadImage( this.props.image )
				.then(
					response => {
						if( this._mounted ){
							this._loading = false
							this._loaded = true
							this.setState({ url: this.props.image, image: response, loading: false })
							this.updateCanvas( response )
						}
					}
				)
		}
	}

    componentWillUnmount(){
        this._mounted = false
        window.removeEventListener("resize", this.handleResize);
        window.removeEventListener("scroll", this.handleScroll);
    }

	componentWillReceiveProps( nextProps ){
		if( ( !this.state.url || nextProps.image != this.state.url ) && !this._loading && nextProps.image ){
			this._loading = true
			this.setState({ url: nextProps.image, image: false, loading: true })
			this.loadImage( nextProps.image )
				.then(
					response => {
						if( this._mounted ){
							this._loading = false
							this._loaded = true
							this.setState({ url: nextProps.image, image: response, loading: false })
							this.updateCanvas( response )
						}
					}
				)
		}
	}

    handleResize(e){
    	if( this._loaded ){
	    	this.updateCanvas( this.state.image );
	    }
    }

    handleScroll(e){
    	// this DOES work, but is in no way high-performing and only on Firefox
    	if( this._loaded ){
			this.setState(
				{ scrollTop: window.scrollY }, 
				this.updateCanvas( this.state.image )
			)
	    }
    }

	loadImage( url ){
		return new Promise( (resolve, reject) => {

			var imageObject = new Image()
			imageObject.src = url
			imageObject.crossOrigin = 'anonymous'

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

	renderCanvas( image = false ){

		if (!image) return null

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
		context.drawImage(image.object, image.x, image.y, image.width, image.height)

		// now update our component
		self.setState({ image: image });
	}

	render(){
		return (
			<div className={this.props.blur ? "parallax blur" : "parallax"}>

				<canvas 
					id="parallax-canvas" 
					className={!this.state.loading ? 'loaded' : null} 
					width={this.state.canvas.width} 
					height={this.state.canvas.height}>
				</canvas>
			</div>
		);
	}
}



