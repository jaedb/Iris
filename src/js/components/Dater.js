
import React, { PropTypes } from 'react'

export default class Dater extends React.Component{

	constructor(props) {
		super(props);
	}

	formatDuration( milliseconds = false ){
		if( !milliseconds ) return null

		var string = '';
		var total_hours, total_minutes, total_seconds, minutes, seconds;

		// get total values for each 
		total_seconds = Math.floor( milliseconds / 1000 )
		total_minutes = Math.floor( milliseconds / (1000 * 60) )
		total_hours = Math.floor(milliseconds / (1000 * 60 * 60))

		// get left-over number of seconds
		seconds = total_seconds - ( total_minutes * 60 )
		if( seconds <= 9 ) seconds = '0'+ seconds

		// get left-over number of minutes
		minutes = total_minutes - ( total_hours * 60 )
		if( minutes <= 9 && total_hours ) minutes = '0'+ minutes
		if( minutes == 0 ) minutes = '0'

		if( total_hours ) string += total_hours+':'
		if( minutes ) string += minutes+':'
		if( seconds ) string += seconds
			
		return string
	}

	calculate(){
		switch(this.props.type){

			case 'total-time':
				var duration = 0;
				var tracks = this.props.data
				for( var i = 0; i < tracks.length; i++ ){
					if( tracks[i].duration_ms ) duration += parseInt(tracks[i].duration_ms);
					if( tracks[i].length ) duration += parseInt(tracks[i].length);
				}
				return this.formatDuration(duration)
				break

			case 'length':
				return this.formatDuration(this.props.data)
				break

			case 'date':
				var date = new Date(this.props.data)
				return date.getDate()+'/'+date.getMonth()+'/'+date.getFullYear()
				break

			case 'ago':
				var date = new Date(this.props.data)
				var diff = new Date() - date
				// todo: calculate nice diff
				return diff
				break

			default:
				return null
		}
	}

	render(){
		return <span className="dater">{ this.calculate() }</span>
	}
}