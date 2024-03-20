import { discordUrl, libraryName, privacyUrl } from '../fixtures/onboarding.json';

describe('Onboarding', () => {
	// TODO: Create debug flag to bypass auto language detection
	it('Alpha onboarding', () => {
		cy.visit('/', {
			onBeforeLoad(win) {
				cy.stub(win, 'open').as('winOpen');
			}
		});

		// Check redirect to initial alpha onboarding screen
		cy.url().should('match', /\/onboarding\/alpha$/);

		// Check application name is present
		cy.get('h1').should('contain', 'Spacedrive');

		// Check logo image exists and loaded correctly
		cy.get('img[alt="Spacedrive"]')
			.should('be.visible')
			.and('have.prop', 'naturalWidth')
			.should('be.greaterThan', 0);

		// Check we are in the alpha release screen
		cy.get('h1').should('contain', 'Alpha Release');

		// Check Join Discord button exists and point to a valid discord invite
		cy.get('button').contains('Join Discord').click();
		cy.get('@winOpen').should('be.calledWith', discordUrl);

		// Check we have a button to continue to the Library creation
		cy.get('a')
			.contains('Continue')
			.should('have.attr', 'href', '/onboarding/new-library')
			.click();

		// Check we were redirect to Library creation screen
		cy.url().should('match', /\/onboarding\/new-library$/);

		// Check create library screen title
		cy.get('h2').should('contain', 'Create a Library');

		// Check we have a button to create a new library
		cy.get('button').contains('New library').as('newLibraryButton');

		// Check we have an input to set the library name
		cy.get('input[placeholder="e.g. \\"James\' Library\\""]').as('libraryNameInput');

		// Check newLibraryButton is disabled
		cy.get('@newLibraryButton').should('be.disabled');

		// Get input with placeholder 'e.g. "James' Library"'
		cy.get('@libraryNameInput').type(libraryName);

		// Check newLibraryButton is enabled
		cy.get('@newLibraryButton').should('be.enabled');

		// Check we can clear the input and the button is disabled again
		cy.get('@libraryNameInput').clear();
		cy.get('@newLibraryButton').should('be.disabled');
		cy.get('@libraryNameInput').type(libraryName);

		// Check we have a button to continue to the add default locations screen
		cy.get('@newLibraryButton').click();

		// Check redirect to add default locations
		cy.url().should('match', /\/onboarding\/locations$/);

		// Check we have a Toggle All button
		cy.get('#toggle-all').as('toggleAllButton');

		cy.get('[data-locations]').then((locationsElem) => {
			const locations = locationsElem.data('locations');
			if (locations == null || typeof locations !== 'object')
				throw new Error('Invalid locations data');

			// Check that default location checkboxes work
			for (const state of ['unchecked', 'checked']) {
				if (state === 'checked') {
					// Check if @toggleAllButton has data-state == checked
					cy.get('@toggleAllButton').should('have.attr', 'data-state', 'checked');
					// Uncheck all locations
					cy.get('@toggleAllButton').click();
				}

				// Check we have all the default locations available
				for (const [location, locationName] of Object.entries(locations)) {
					if (typeof locationName !== 'string') throw new Error('Invalid location name');

					let newState: typeof state;
					if (state === 'unchecked') {
						cy.get('label').contains(locationName).click();
						newState = 'checked';
					} else {
						newState = 'unchecked';
					}
					cy.get(`button[id="locations.${location}"]`).should(
						'have.attr',
						'data-state',
						newState
					);
				}
			}
		});

		// Check we have a button to continue to the privacy screen
		cy.get('button').contains('Continue').click();

		// Check redirect to privacy screen
		cy.url().should('match', /\/onboarding\/privacy$/);

		// Check privacy screen title
		cy.get('h2').should('contain', 'Your Privacy');

		// Check we have all privacy options
		cy.get('label').contains('Share the bare minimum').click();
		cy.get('#radiominimal-telemetry').should('have.attr', 'data-state', 'checked');
		cy.get('#radioshare-telemetry').should('have.attr', 'data-state', 'unchecked');
		cy.get('label').contains('Share anonymous usage').click();
		cy.get('#radioshare-telemetry').should('have.attr', 'data-state', 'checked');
		cy.get('#radiominimal-telemetry').should('have.attr', 'data-state', 'unchecked');

		// Check More info button exists and point to the valid pravacy policy
		cy.get('button').contains('More info').click();
		cy.get('@winOpen').should('be.calledWith', privacyUrl);

		// Check we have a button to finish onboarding
		cy.get('button[type="submit"]').contains('Continue').click();

		// Check redirect to privacy screen
		cy.url().should('match', /\/onboarding\/creating-library$/);

		// FIX-ME: This fails a lot, due to the creating library screen only being show for a short time
		// Check creating library screen title
		// cy.get('h2').should('contain', 'Creating your library');

		// Check redirect to Library
		cy.checkUrlIsLibrary();

		// Delete library
		cy.deleteLibrary(libraryName);
	});
});
