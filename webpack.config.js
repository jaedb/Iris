var dev = process.env.NODE_ENV !== "production";
var webpack = require('webpack');
var ExtractTextPlugin = require('extract-text-webpack-plugin');

var node_dir = __dirname + '/node_modules';
var output_dir = __dirname +"/production"

var config = {
	
	context: __dirname,
	entry: "./src/index.js",
	
	output: {
		path: output_dir,
		filename: 'app.js'
	},
	
	module: {
		loaders: [
			{ test: require.resolve('jquery'), loader: 'expose?jQuery!expose?$' },
			{
				// loading JSX (aka Babel) into browser-friendly ES6
				test: /\.js$/,
				exclude: [
					/(node_modules|bower_components)/,
					'index.js',
				],
				loader: 'babel-loader',
				query: {
					presets: ['es2015', 'react']
				}
			},
			{
				// loading sass asset files
				test: /\.scss$/,
				loader: ExtractTextPlugin.extract([
					'css'+(dev? '?sourceMap=true': ''),
					'sass'+(dev? '?outputStyle=expanded&sourceMap=true&sourceMapContents=true': '')
				])
			}
		]
	},
	
	plugins: [
		new webpack.optimize.DedupePlugin(),
		new webpack.optimize.OccurenceOrderPlugin(),
		new webpack.ProvidePlugin({
		    $: "jquery",
		    jQuery: "jquery",
		    "window.jQuery": "jquery"
		})
	]
};

/**
 * Development-only configuration values
 **/
if( dev ){
	
	// set compiled css location
	config.plugins.push( new ExtractTextPlugin("app.css") );
	
	// we want source maps
	config.devtool = 'source-map';
	
	
/**
 * Production-only configuration values
 **/
}else{
	
	// set our final output filename
	config.output.filename = 'app.min.js';
	
	// re-iterate our production value as a string (for ReactJS building)
	config.plugins.push(
		new webpack.DefinePlugin({
			'process.env':{
				'NODE_ENV': JSON.stringify('production')
			}
		})
	);
	
	// remove all debug and console code
	config.module.loaders.push(
		{ 
			test: /\.(js|jsx)$/,
			loader: "webpack-strip?strip[]=console.log,strip[]=console.info,strip[]=debug"
		}
	);
	
	// set compiled css location
	config.plugins.push( new ExtractTextPlugin("app.min.css") );
	
	// uglify our js, with no sourcemaps
	config.plugins.push(
			new webpack.optimize.UglifyJsPlugin({
				compress: true,
				mangle: false,
				sourceMap: false,
				comments: false
			})
		);
}

// now export our collated config object
module.exports = config;
