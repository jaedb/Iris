
import React, { PropTypes } from 'react'
import * as actions from '../actions'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

class Todos extends React.Component{

	constructor(props) {
		super(props);
	}

	// on load of this component
	componentDidMount(){
		this.props.actions.load();
	}

	render(){
		return (
			<Todos_View todos={this.props.todos} actions={this.props.actions} />
		);
	}
}



/**
 * The Todo's presenter, or view
 *
 * Any UI and UX display occurs here, and is passed back to it's brain through props
 * This allows for reuse of the brain for different contexts without duplicating functionality
 **/
class Todos_View extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<div>
				<ul className="todos">
					{
						this.props.todos.map(todo =>
							<TodoItem todo={todo} key={todo.id} actions={this.props.actions} />
						)
					}
				</ul>
				<AddTodoForm actions={this.props.actions} />
			</div>
		);
	}
}


/**
 * The Todo item
 **/
class TodoItem extends React.Component{

	constructor(props) {
		super(props);
        this.handleClick = this.handleClick.bind(this);
	}

	handleClick(){
        this.props.actions.toggleDone(this.props.todo.id);
        this.props.actions.save();
	}

	render(){
		var style = {};
		if( this.props.todo.completed ) style.textDecoration = 'line-through';
		return <li onClick={this.handleClick} style={style}>{ this.props.todo.title }</li>;
	}
}


/**
 * The Todo item
 **/
class AddTodoForm extends React.Component{

  constructor(props) {
    super(props);

    this.state = {
      title: ''
    }
    this.handleChange = this.handleChange.bind(this);
    this.handleSubmit = this.handleSubmit.bind(this);
  }

  handleSubmit(){
    this.props.actions.addTodo( this.state.title );
    this.props.actions.save();
  	this.setState({ title: '' });
  }

	handleChange( newTitle ){
		this.setState({ title: newTitle });
	}

	render(){
        return <AddTodoForm_View title={this.state.title} handleChange={this.handleChange} handleSubmit={this.handleSubmit} />
	}
}

class AddTodoForm_View extends React.Component{

	constructor(props) {
		super(props);
		this.handleChange = this.handleChange.bind(this);
		this.handleSubmit = this.handleSubmit.bind(this);
	}

	handleChange( event ){
		this.props.handleChange( event.target.value );
	}

	handleSubmit( event ){
		event.preventDefault();
		this.props.handleSubmit();
	}

	render(){
		return (
			<form onSubmit={ this.handleSubmit }>
				<input type="text" value={ this.props.title } onChange={this.handleChange} />
				<input type="submit" value="Add" />
			</form>
		);
	}
}

/*
Todos.propTypes = {
	todos: React.PropTypes.array.isRequired
};
*/

/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Todos)