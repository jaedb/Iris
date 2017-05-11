
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

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
	}

	componentWillUnmount(){		
		window.removeEventListener("click", this.handleClick, false)
	}

	handleClick(e){
		if ($(e.target).closest('.autocomplete-field').length <= 0){
			this.props.spotifyActions.clearAutocompleteResults()
		}
	}

	handleChange(value){
		var self = this
		clearTimeout(this.timer)
		this.setState({value: value})
        this.timer = setTimeout(
            function(){
            	self.setState({searching: true})
                self.props.spotifyActions.getAutocompleteResults(self.id,value,self.props.types)
            },
            500
        )
	}

	handleSelect(item){
		if (this.props.clearOnSelect){
			this.setState({value: ''})
		} else {
			this.setState({value: item.name})
		}

		this.props.handleSelect(item)
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
							<div className="result" key={item.uri} onClick={() => this.handleSelect(item)}>
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
						onChange={e => this.handleChange(e.target.value)} 
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
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		results: (state.spotify.autocomplete_results ? state.spotify.autocomplete_results : {})
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AutocompleteField)