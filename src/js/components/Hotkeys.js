import React from 'react';

export default class Hotkeys extends React.Component {
    
	constructor(props){
		super(props);
		this.handleKeyDown = this.handleKeyDown.bind(this);
	}

	componentWillMount(){
		window.addEventListener("keydown", this.handleKeyDown, false);
	}

	componentWillUnmount(){
		window.removeEventListener("keydown", this.handleKeyDown, false);
	}

	handleKeyDown(e) {	

		// When we're focussed on certian elements, don't fire any shortcuts
		// Typically form inputs
		let ignoreNodes = ['INPUT', 'TEXTAREA'];
		if (ignoreNodes.indexOf(e.target.nodeName) > -1){
			return;
        }

        // Ignore when there are any key modifiers. This enables us to avoid interfering
        // with browser- and OS-default functions.
        if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey){
            return;
        }

		switch(e.key.toLowerCase()){

			case " ":
				if (this.props.play_state == 'playing'){
					this.props.mopidyActions.pause();
					this.props.uiActions.createNotification({content: 'pause', type: 'shortcut'});
				} else {
					this.props.mopidyActions.play();
					this.props.uiActions.createNotification({content: 'play_arrow', type: 'shortcut'});
				}
				break;

			case "escape":
				if (this.props.dragging){
					this.props.uiActions.dragEnd();
				} else if (this.props.modal){
			        window.history.back();
                }
                break;

			case "s":
                this.props.history.push('/search');
                break;

            case "c":
                this.props.history.push('/queue');
                break;

            case "k":
                this.props.history.push('/kiosk-mode');
                break;

			case ",":
                window.history.back();
            break;
    
			case ".":
                window.history.forward();
                break;
            
            case "l":
                var volume = this.props.volume
                if (volume !== 'false'){
                    volume += 5;
                    if (volume > 100){
                        volume = 100
                    }
                    this.props.mopidyActions.setVolume(volume);
                    if (this.props.mute){
                        this.props.mopidyActions.setMute(false);
                    }
                this.props.uiActions.createNotification({content: 'volume_up', type: 'shortcut'});
                }
                break;

            case "q":
                var volume = this.props.volume;
                if (volume !== 'false'){
                    volume -= 5;
                    if (volume < 0){
                        volume = 0;
                    }
                    this.props.mopidyActions.setVolume(volume);
                    if (this.props.mute){
                        this.props.mopidyActions.setMute(false);
                    }
                }
                this.props.uiActions.createNotification({content: 'volume_down', type: 'shortcut'});
				break;

            case "m":
                if (this.props.mute){
                    this.props.mopidyActions.setMute(false);
                    this.props.uiActions.createNotification({content: 'volume_up', type: 'shortcut'});
                } else {
                    this.props.mopidyActions.setMute(true);
                    this.props.uiActions.createNotification({content: 'volume_off', type: 'shortcut'});
                }
                break;

			case "r":
                var new_position = this.props.play_time_position - 30000;
                if (new_position < 0){
                    new_position = 0;;
                }
                this.props.mopidyActions.setTimePosition(new_position);
                this.props.uiActions.createNotification({content: 'fast_rewind', type: 'shortcut'});
                break;

			case "f":
                this.props.mopidyActions.setTimePosition(this.props.play_time_position + 30000);
                this.props.uiActions.createNotification({content: 'fast_forward', type: 'shortcut'});
                break;
            
            case "p":
                this.props.mopidyActions.previous();
                this.props.uiActions.createNotification({content: 'skip_previous', type: 'shortcut'});
                break;

            case "n":
                this.props.mopidyActions.next();
                this.props.uiActions.createNotification({content: 'skip_next', type: 'shortcut'});
				break;
        }
        
        e.preventDefault();
        return false;
    }

    render(){
        return null;
    }
}