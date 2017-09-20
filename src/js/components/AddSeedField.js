
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'

import ArtistSentence from './ArtistSentence'
import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as spotifyActions from '../services/spotify/actions'

class AddSeedField extends React.Component{

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

		if (!this.props.genres){
			this.props.spotifyActions.getGenres()
		}
	}

	componentWillUnmount(){		
		window.removeEventListener("click", this.handleClick, false)
	}

	handleClick(e){
		if ($(e.target).closest('.add-seed-field').length <= 0){
			this.props.spotifyActions.clearAutocompleteResults(this.id)
		}
	}

	handleChange(e,value){
		var self = this

		// update our local state
		this.setState({value: value})
		
		// start a timer to perform the actual search
		// this provides a wee delay between key presses to avoid request spamming
		clearTimeout(this.timer)
        this.timer = setTimeout(
            function(){
            	self.setState({searching: true})
                self.props.spotifyActions.getAutocompleteResults(self.id,value,['artist','track','genre'])
            },
            500
        )
	}

	handleSelect(e,item){
		this.setState({value: ''})
		this.props.onSelect(e,item.uri)
		this.props.spotifyActions.clearAutocompleteResults(this.id)

		// Add our selected item to our global index
		switch (helpers.uriType(item.uri)){

			case 'artist':
				this.props.coreActions.artistLoaded(item.uri,item)
				break

			case 'track':
				this.props.coreActions.trackLoaded(item.uri,item)
				break
		}
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
		var className = "field autocomplete-field add-seed-field"
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
					{this.renderResults('artists')}
					{this.renderResults('tracks')}
					{this.renderResults('genres')}
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		genres: (state.ui.genres ? state.ui.genres : null),
		results: (state.spotify.autocomplete_results ? state.spotify.autocomplete_results : {})
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AddSeedField)