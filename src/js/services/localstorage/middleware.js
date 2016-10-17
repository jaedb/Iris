
const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        // get the state, and plug it in to our localStorage
        var state = store.getState();
        localStorage.setItem('state', JSON.stringify(state));
    }

})();

export default localstorageMiddleware