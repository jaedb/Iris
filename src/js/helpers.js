

export let SizedImages = function( images ){

	var sizes = {
		small: false,
		medium: false,
		large: false,
		huge: false
	}

	if( images.length > 0 ){

		for( var i = 0; i < images.length; i++ ){
			if( images[i].height > 800 ){
				sizes.huge = images[i].url;
			}else if( images[i].height > 600 ){
				sizes.large = images[i].url;
			}else if( images[i].height > 280 ){
				sizes.medium = images[i].url;
			}else{
				sizes.small = images[i].url;
			}
		}

		if( !sizes.medium )	sizes.medium = sizes.small;
		if( !sizes.large )	sizes.large = sizes.medium;
		if( !sizes.huge ) 	sizes.huge = sizes.large;
	}

	return sizes;
}