import React from 'react';
import { BrowserRouter } from "react-router-dom";

// Testing-specific
import { shallow, mount, render } from 'enzyme';

// Test subjects
import Dater from '../../src/js/components/Dater';

describe('<Dater />', () => {

	it('should render null when given invalid props', () => {

		// Invalid type
		const dom_1 = shallow(<Dater type="invalid-type" data={100000} />);
		expect(dom_1.type()).toEqual(null);

		// Missing type
		const dom_2 = shallow(<Dater data={100000} />);
		expect(dom_2.type()).toEqual(null);

		// Missing data
		const dom_4 = shallow(<Dater type="ago" />);
		expect(dom_4.type()).toEqual(null);
	});

	it('should handle milliseconds', () => {
		const dom = shallow(<Dater type="length" data={30000} />);
		expect(dom.text()).toEqual('0:30');
	});

	it('should handle date (yyyy-mm-dd)', () => {
		const dom = shallow(<Dater type="date" data="2015-10-23" />);
		expect(dom.text()).toEqual("23/10/2015");
	});

	it('should handle date (mm/dd/yyyy)', () => {
		const dom = shallow(<Dater type="date" data="10/23/2015" />);
		expect(dom.text()).toEqual("23/10/2015");
	});

	it('should handle ago (days)', () => {
		var date = new Date();
		date.setDate( date.getDate() - 3 );

		const dom = shallow(<Dater type="ago" data={date} />);
		expect(dom.text()).toEqual('3 days');
	});

	it('should handle ago (hours)', () => {
		var date = new Date();
		date.setTime( date.getTime() - 3 * 3600000 );

		const dom = shallow(<Dater type="ago" data={date} />);
		expect(dom.text()).toEqual('3 hours');
	});

	it('should handle ago (minutes)', () => {
		var date = new Date();
		date.setTime( date.getTime() - 3 * 60000 );

		const dom = shallow(<Dater type="ago" data={date} />);
		expect(dom.text()).toEqual('3 minutes');
	});

	it('should handle total time', () => {
		var data = [
			{
				duration: 5 * 60000
			},
			{
				duration: 5 * 60000
			},
			{
				duration: 5 * 60000
			}
		];
		const dom = shallow(<Dater type="total-time" data={data} />);
		expect(dom.text()).toEqual('15 mins');
	});
});