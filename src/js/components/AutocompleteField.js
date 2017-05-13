
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'

import ArtistSentence from './ArtistSentence'
import * as helpers from '../helpers'
import * as spotifyActions from '../services/spotify/actions'

class AutocompleteField extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			value: ''
		}
		this.id = helpers.generateGuid()
		this.timer = null
		this.handleClick = this.handleClick.bind(this)
	}

	componentDidMount(){
		window.addEventListener("click", this.handleClick, false)

		if (this.props.types.includes('genre') && !this.props.genres){
			this.props.spotifyActions.getGenres()
		}
	}

	componentWillUnmount(){		
		window.removeEventListener("click", this.handleClick, false)
	}

	handleClick(e){
		if ($(e.target).closest('.autocomplete-field').length <= 0){
			this.props.spotifyActions.clearAutocompleteResults()
		}
	}

	handleChange(e,value){
		var self = this

		// update our local state
		this.setState({value: value})

		// pass the change up the line
		if (this.props.onChange){
			this.props.onChange(e,value)
		}
		
		// start a timer to perform the actual search
		// this provides a wee delay between key presses to avoid request spamming
		clearTimeout(this.timer)
        this.timer = setTimeout(
            function(){
            	self.setState({searching: true})
                self.props.spotifyActions.getAutocompleteResults(self.id,value,self.props.types)
            },
            500
        )
	}

	handleSelect(e,item){
		if (this.props.clearOnSelect){
			this.setState({value: ''})
		} else {
			this.setState({value: item.name})
		}

		// if we have a handler to pass down to
		if (this.props.onSelect){
			this.props.onSelect(e,item)

		// no handler, so let's just go to this asset
		} else {
			switch (helpers.uriType(item.uri)){

				case 'album':
					hashHistory.push(global.baseURL+'album/'+item.uri)
					break

				case 'artist':
					hashHistory.push(global.baseURL+'artist/'+item.uri)
					break

				case 'playlist':
					hashHistory.push(global.baseURL+'playlist/'+item.uri)
					break

				case 'track':
					hashHistory.push(global.baseURL+'album/'+item.album.uri)
					break

				case 'search':
					hashHistory.push(global.baseURL+'search/'+item.uri)
					break
			}
		}

		this.props.spotifyActions.clearAutocompleteResults(this.id)
	}

	results(){
		if (typeof(this.props.results) === 'undefined'){
			return null
		} else if (typeof(this.props.results[this.id]) === 'undefined'){
			return null
		} else {
			return this.props.results[this.id]
		}
	}

	renderAllResultsButton(){
		 if (this.props.hideAllResultsButton || !this.results()){
		 	return null
		 } else {
			return (
				<div className="all-results" onClick={e => this.handleSelect(e,{uri:'iris:search:all:'+this.state.value, name: ''})}>
					All results
				</div>
			)
		 }
	}

	renderResults(type){
		var results = this.results()
		if (!results || typeof(results[type]) === 'undefined' || results[type].length <= 0) return null
		
		// only show the first 3
		var items = results[type].slice(0,3)

		return (
			<div className="type">
				<h4 className="grey-text">{type}</h4>
				{
					items.map(item => {
						return (
							<div className="result" key={item.uri} onClick={e => this.handleSelect(e,item)}>
								{item.name}
								{type == 'tracks' ? <span className="grey-text"> <ArtistSentence artists={item.artists} nolinks /></span> : null}
							</div>
						)
					})
				}
			</div>
		)
	}

	render(){
		var className = "field autocomplete-field"
		if (this.results() && this.results().loading){
			className += " loading"
		}
		return (
			<div className={className}>
				<div className="input">
					<input 
						type="text" 
						value={this.state.value}
						onChange={e => this.handleChange(e,e.target.value)} 
						placeholder={this.props.placeholder ? this.props.placeholder : "Start typing..."} />
				</div>
				<div className="results">
					{
						this.props.types.map(type => {
							return (
								<div key={type}>
									{this.renderResults(type+'s')}
								</div>
							)
						})
					}
					{this.renderAllResultsButton()}
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		genres: (state.spotify.genres ? state.spotify.genres : null),
		results: (state.spotify.autocomplete_results ? state.spotify.autocomplete_results : {})
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AutocompleteField)