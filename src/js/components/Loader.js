
import React, { memo } from 'react';
import Icon from './Icon';

export default memo((props) => {
    const { body, loading, mini, lazy, white, className = '', children = null } = props;

    if (!loading) {
        return null;
    }

    let classNameString = 'loader';
    if (className) {
        classNameString += ` ${className}`;
    }
    if (mini) {
        classNameString += ` loader--mini`;
    }
    if (body) {
        classNameString += ` loader--body`;
    }
    if (lazy) {
        classNameString += ` loader--lazy`;
    }
    if (white) {
        classNameString += ` loader--white`;
    }

    return (
        <div className={classNameString}>
            {
                navigator.onLine ? (
                    <div className="loader__spinner"></div>
                ) : (                    
                    <div className="loader__offline">
                        <Icon name="wifi_off" />
                        <p>You need to be online load this resource</p>
                    </div>
                )
            }
            {children}
        </div>
    );
});
