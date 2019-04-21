
const isDev = process.env.NODE_ENV !== "production";
const path = require('path');
const webpack = require('webpack');
const MiniCssExtractPlugin = require('mini-css-extract-plugin');

const config = {
	mode: process.env.NODE_ENV,
	context: path.resolve(__dirname),
	entry: {
		js: './src/js/index'
	},
	output: {
		path: path.resolve(__dirname, 'mopidy_iris/static'),
		filename: 'app'+(isDev ? '' : '.min')+'.js'
	},
	module: {
		rules: [
			{
				test: require.resolve('jquery'),
        		exclude: [
        			/node_modules/
        		],
				use: [
					'expose-loader?jQuery!expose?$'
				]
			},
			{
				test: /\.js$/,
				exclude: /node_modules/,
				use: [
					{
						loader: 'babel-loader',
						options: {
							presets: [
								'react',
								'es2015',
								'stage-2'
							]
						}
					}
				]
			},
			{
				test: /\.scss$/,
				use: [
					{
						loader: MiniCssExtractPlugin.loader,
						options: {
							publicPath: './',
							hmr: process.env.NODE_ENV === 'development',
						}
					},
					'css-loader',
					'sass-loader',
				]
			},
			{
				// load external resources (ie Google fonts)
				test: /.(gif|png|woff(2)?|eot|ttf|svg)(\?[a-z0-9=\.]+)?$/,
				use: [
					{ 
						loader: 'url-loader',
						options: {
							name: 'assets/fonts/[name].[ext]?[hash]',
							limit: 100000
						}
					}
				]
			},
			(isDev ? {} : {
				test: /\.(js)$/,
				use: [
					{
						loader: 'webpack-strip',
						options: {
							strip: [
								'console.log',
								'console.info',
								'debug'
							]
						}
					}
				]
			})
		]
	},
	plugins: [
		new webpack.ProvidePlugin({
		    $: "jquery",
		    jQuery: "jquery",
		    "window.jQuery": "jquery"
		}),
		new MiniCssExtractPlugin({
			filename: 'app'+(isDev ? '' : '.min')+'.css',
		}),
	],
	watchOptions: {
		poll: true
	},
	devtool: (isDev ? 'source-map': false)
};

// now export our collated config object
module.exports = config;
