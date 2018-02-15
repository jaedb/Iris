
import React, { PropTypes } from 'react'

export default class Icon extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			data: null
		}
	}

	componentDidMount(){
		this.load();
	}

	componentWillReceiveProps(newProps){
		if (this.props.name !== newProps.name){
			this.load();
		}
	}

	load(){		
		var self = this;
		var url = 'assets/icons/'+this.props.name+'.svg';
		var xmlHttp = new XMLHttpRequest();

		xmlHttp.onreadystatechange = function(){ 
			if (xmlHttp.readyState == 4 && xmlHttp.status == 200){
				self.setState({data: xmlHttp.responseText});
			}
		}
		xmlHttp.open("GET", url, true);
		xmlHttp.send(null);
	}

	render(){
		var className = 'icon';
		if (this.props.className){
			className += ' '+this.props.className;
		}

		return <span className={className} dangerouslySetInnerHTML={{ __html: this.state.data}}></span>;
	}
}