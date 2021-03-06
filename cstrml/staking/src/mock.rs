//! Test utilities

use crate::{
    inflation, EraIndex, GenesisConfig, Module, Nominators, RewardDestination, StakerStatus, Trait,
    ValidatorPrefs,
};
use frame_support::{
    assert_ok, impl_outer_origin, parameter_types,
    traits::{Currency, FindAuthor, Get},
    weights::Weight,
    StorageLinkedMap, StorageValue,
};
use sp_core::{crypto::key_types, H256};
use sp_io;
use sp_runtime::curve::PiecewiseLinear;
use sp_runtime::testing::{Header, UintAuthorityId};
use sp_runtime::traits::{Convert, IdentityLookup, OnInitialize, OpaqueKeys, SaturatedConversion};
use sp_runtime::{KeyTypeId, Perbill};
use sp_staking::{
    offence::{OffenceDetails, OnOffenceHandler},
    SessionIndex,
};
use std::{cell::RefCell, collections::HashSet};

/// The AccountId alias in this test module.
pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;

/// Simple structure that exposes how u64 currency can be represented as... u64.
pub struct CurrencyToVoteHandler;
impl Convert<u64, u64> for CurrencyToVoteHandler {
    fn convert(x: u64) -> u64 {
        x
    }
}
impl Convert<u128, u64> for CurrencyToVoteHandler {
    fn convert(x: u128) -> u64 {
        x.saturated_into()
    }
}

thread_local! {
    static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
    static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(0);
    static SLASH_DEFER_DURATION: RefCell<EraIndex> = RefCell::new(0);
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AccountId> for TestSessionHandler {
    const KEY_TYPE_IDS: &'static [KeyTypeId] = &[key_types::DUMMY];

    fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(AccountId, Ks)]) {}

    fn on_new_session<Ks: OpaqueKeys>(
        _changed: bool,
        validators: &[(AccountId, Ks)],
        _queued_validators: &[(AccountId, Ks)],
    ) {
        SESSION.with(|x| {
            *x.borrow_mut() = (
                validators.iter().map(|x| x.0.clone()).collect(),
                HashSet::new(),
            )
        });
    }

    fn on_disabled(validator_index: usize) {
        SESSION.with(|d| {
            let mut d = d.borrow_mut();
            let value = d.0[validator_index];
            d.1.insert(value);
        })
    }
}

pub fn is_disabled(controller: AccountId) -> bool {
    let stash = Staking::ledger(&controller).unwrap().stash;
    SESSION.with(|d| d.borrow().1.contains(&stash))
}

pub struct ExistentialDeposit;
impl Get<u64> for ExistentialDeposit {
    fn get() -> u64 {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
    }
}

pub struct SlashDeferDuration;
impl Get<EraIndex> for SlashDeferDuration {
    fn get() -> EraIndex {
        SLASH_DEFER_DURATION.with(|v| *v.borrow())
    }
}

impl_outer_origin! {
    pub enum Origin for Test  where system = frame_system {}
}

/// Author of block is always 11
pub struct Author11;
impl FindAuthor<u64> for Author11 {
    fn find_author<'a, I>(_digests: I) -> Option<u64>
    where
        I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
    {
        Some(11)
    }
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for Test {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Call = ();
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type AvailableBlockRatio = AvailableBlockRatio;
    type MaximumBlockLength = MaximumBlockLength;
    type Version = ();
    type ModuleToIndex = ();
}
parameter_types! {
    pub const TransferFee: Balance = 0;
    pub const CreationFee: Balance = 0;
}
impl pallet_balances::Trait for Test {
    type Balance = Balance;
    type OnFreeBalanceZero = Staking;
    type OnNewAccount = ();
    type Event = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
}
parameter_types! {
    pub const Period: BlockNumber = 1;
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
impl pallet_session::Trait for Test {
    type OnSessionEnding = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
    type Keys = UintAuthorityId;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = TestSessionHandler;
    type Event = ();
    type ValidatorId = AccountId;
    type ValidatorIdOf = crate::StashOf<Test>;
    type SelectInitialValidators = Staking;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl pallet_session::historical::Trait for Test {
    type FullIdentification = crate::Exposure<AccountId, Balance>;
    type FullIdentificationOf = crate::ExposureOf<Test>;
}
impl pallet_authorship::Trait for Test {
    type FindAuthor = Author11;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = Module<Test>;
}
parameter_types! {
    pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
}
impl tee::Trait for Test {
    type Event = ();
}
pallet_staking_reward_curve::build! {
    const I_NPOS: PiecewiseLinear<'static> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_100_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}
parameter_types! {
    pub const SessionsPerEra: SessionIndex = 3;
    pub const BondingDuration: EraIndex = 3;
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
}
impl Trait for Test {
    type Currency = pallet_balances::Module<Self>;
    type Time = pallet_timestamp::Module<Self>;
    type CurrencyToVote = CurrencyToVoteHandler;
    type RewardRemainder = ();
    type Event = ();
    type Slash = ();
    type Reward = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type SessionInterface = Self;
    type RewardCurve = RewardCurve;
}

pub struct ExtBuilder {
    existential_deposit: u64,
    validator_pool: bool,
    nominate: bool,
    validator_count: u32,
    minimum_validator_count: u32,
    slash_defer_duration: EraIndex,
    fair: bool,
    num_validators: Option<u32>,
    invulnerables: Vec<u64>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            existential_deposit: 0,
            validator_pool: false,
            nominate: true,
            validator_count: 2,
            minimum_validator_count: 0,
            slash_defer_duration: 0,
            fair: true,
            num_validators: None,
            invulnerables: vec![],
        }
    }
}

impl ExtBuilder {
    pub fn existential_deposit(mut self, existential_deposit: u64) -> Self {
        self.existential_deposit = existential_deposit;
        self
    }
    pub fn validator_pool(mut self, validator_pool: bool) -> Self {
        self.validator_pool = validator_pool;
        self
    }
    pub fn nominate(mut self, nominate: bool) -> Self {
        self.nominate = nominate;
        self
    }
    pub fn validator_count(mut self, count: u32) -> Self {
        self.validator_count = count;
        self
    }
    pub fn minimum_validator_count(mut self, count: u32) -> Self {
        self.minimum_validator_count = count;
        self
    }
    pub fn slash_defer_duration(mut self, eras: EraIndex) -> Self {
        self.slash_defer_duration = eras;
        self
    }
    pub fn fair(mut self, is_fair: bool) -> Self {
        self.fair = is_fair;
        self
    }
    pub fn num_validators(mut self, num_validators: u32) -> Self {
        self.num_validators = Some(num_validators);
        self
    }
    pub fn invulnerables(mut self, invulnerables: Vec<u64>) -> Self {
        self.invulnerables = invulnerables;
        self
    }
    pub fn set_associated_consts(&self) {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow_mut() = self.existential_deposit);
        SLASH_DEFER_DURATION.with(|v| *v.borrow_mut() = self.slash_defer_duration);
    }
    pub fn build(self) -> sp_io::TestExternalities {
        self.set_associated_consts();
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        let balance_factor = if self.existential_deposit > 0 { 256 } else { 1 };

        let num_validators = self.num_validators.unwrap_or(self.validator_count);
        let validators = (0..num_validators)
            .map(|x| ((x + 1) * 10 + 1) as u64)
            .collect::<Vec<_>>();

        let _ = pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 10 * balance_factor),
                (2, 20 * balance_factor),
                (3, 300 * balance_factor),
                (4, 400 * balance_factor),
                (10, balance_factor),
                (11, balance_factor * 1000),
                (20, balance_factor),
                (21, balance_factor * 2000),
                (30, balance_factor),
                (31, balance_factor * 2000),
                (40, balance_factor),
                (41, balance_factor * 2000),
                (100, 2000 * balance_factor),
                (101, 2000 * balance_factor),
                // This allow us to have a total_payout different from 0.
                (999, 1_000_000_000_000),
            ],
            vesting: vec![],
        }
        .assimilate_storage(&mut storage);

        let stake_21 = if self.fair { 1000 } else { 2000 };
        let stake_31 = if self.validator_pool {
            balance_factor * 1000
        } else {
            1
        };
        let status_41 = if self.validator_pool {
            StakerStatus::<AccountId, Balance>::Validator
        } else {
            StakerStatus::<AccountId, Balance>::Idle
        };
        let nominated = if self.nominate {
            vec![(11, 250), (21, 250)]
        } else {
            vec![]
        };

        // tee genesis
        let identities: Vec<u64> = vec![10, 20, 30, 40, 2, 60, 50, 70, 4, 6, 100];

        let _ = GenesisConfig::<Test> {
            current_era: 0,
            stakers: vec![
                // (stash, controller, staked_amount, status)
                (
                    11,
                    10,
                    balance_factor * 1000,
                    StakerStatus::<AccountId, Balance>::Validator,
                ),
                (
                    21,
                    20,
                    stake_21,
                    StakerStatus::<AccountId, Balance>::Validator,
                ),
                (
                    31,
                    30,
                    stake_31,
                    StakerStatus::<AccountId, Balance>::Validator,
                ),
                (41, 40, balance_factor * 1000, status_41),
                // nominator
                (
                    101,
                    100,
                    balance_factor * 500,
                    StakerStatus::<AccountId, Balance>::Nominator(nominated),
                ),
            ],
            validator_count: self.validator_count,
            minimum_validator_count: self.minimum_validator_count,
            invulnerables: self.invulnerables,
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_session::GenesisConfig::<Test> {
            keys: validators
                .iter()
                .map(|x| (*x, UintAuthorityId(*x)))
                .collect(),
        }
        .assimilate_storage(&mut storage);

        let _ = tee::GenesisConfig::<Test> {
            tee_identities: identities
                .iter()
                .map(|id| (*id, Default::default()))
                .collect(),
            work_reports: identities
                .iter()
                .map(|id| {
                    (
                        *id,
                        tee::WorkReport {
                            pub_key: vec![],
                            block_number: 0,
                            block_hash: vec![],
                            empty_root: vec![],
                            empty_workload: 20000000000000,
                            meaningful_workload: 0,
                            sig: vec![],
                        },
                    )
                })
                .collect(),
        }
        .assimilate_storage(&mut storage);

        let mut ext = sp_io::TestExternalities::from(storage);
        ext.execute_with(|| {
            let validators = Session::validators();
            SESSION.with(|x| *x.borrow_mut() = (validators.clone(), HashSet::new()));
        });
        ext
    }
}

pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type Session = pallet_session::Module<Test>;
pub type Timestamp = pallet_timestamp::Module<Test>;
pub type Staking = Module<Test>;

pub fn check_exposure_all() {
    Staking::current_elected()
        .into_iter()
        .for_each(|acc| check_exposure(acc));
}

pub fn check_nominator_all() {
    <Nominators<Test>>::enumerate().for_each(|(acc, _)| check_nominator_exposure(acc));
}

/// Check for each selected validator: expo.total = Sum(expo.other) + expo.own
pub fn check_exposure(stash: u64) {
    assert_is_stash(stash);
    let expo = Staking::stakers(&stash);
    assert_eq!(
        expo.total as u128,
        expo.own as u128 + expo.others.iter().map(|e| e.value as u128).sum::<u128>(),
        "wrong total exposure for {:?}: {:?}",
        stash,
        expo,
    );
}

/// Check that for each nominator: slashable_balance > sum(used_balance)
/// Note: we might not consume all of a nominator's balance, but we MUST NOT over spend it.
pub fn check_nominator_exposure(stash: u64) {
    assert_is_stash(stash);
    let mut sum = 0;
    Staking::current_elected()
        .iter()
        .map(|v| Staking::stakers(v))
        .for_each(|e| {
            e.others
                .iter()
                .filter(|i| i.who == stash)
                .for_each(|i| sum += i.value)
        });
    let nominator_stake = Staking::slashable_balance_of(&stash);
    // a nominator cannot over-spend.
    assert!(
        nominator_stake >= sum,
        "failed: Nominator({}) stake({}) >= sum divided({})",
        stash,
        nominator_stake,
        sum,
    );
}

pub fn assert_is_stash(acc: u64) {
    assert!(Staking::bonded(&acc).is_some(), "Not a stash.");
}

pub fn assert_ledger_consistent(stash: u64) {
    assert_is_stash(stash);
    let ledger = Staking::ledger(stash - 1).unwrap();

    let real_total: Balance = ledger
        .unlocking
        .iter()
        .fold(ledger.active, |a, c| a + c.value);
    assert_eq!(real_total, ledger.total);
}

pub fn bond_validator(acc: u64, val: u64) {
    // a = controller
    // a + 1 = stash
    let _ = Balances::make_free_balance_be(&(acc + 1), val);
    assert_ok!(Staking::bond(
        Origin::signed(acc + 1),
        acc,
        val,
        RewardDestination::Controller
    ));
    Staking::upsert_stake_limit(&(acc + 1), u64::max_value());
    assert_ok!(Staking::validate(
        Origin::signed(acc),
        ValidatorPrefs::default()
    ));
}

pub fn bond_nominator(acc: u64, val: u64, target: Vec<(u64, u64)>) {
    // a = controller
    // a + 1 = stash
    let _ = Balances::make_free_balance_be(&(acc + 1), val);
    assert_ok!(Staking::bond(
        Origin::signed(acc + 1),
        acc,
        val,
        RewardDestination::Controller
    ));
    assert_ok!(Staking::nominate(Origin::signed(acc), target));
}

pub fn advance_session() {
    let current_index = Session::current_index();
    start_session(current_index + 1);
}

pub fn start_session(session_index: SessionIndex) {
    // Compensate for session delay
    let session_index = session_index + 1;
    for i in Session::current_index()..session_index {
        System::set_block_number((i + 1).into());
        Timestamp::set_timestamp(System::block_number() * 1000);
        Session::on_initialize(System::block_number());
    }

    assert_eq!(Session::current_index(), session_index);
}

pub fn start_era(era_index: EraIndex) {
    start_session((era_index * 3).into());
    assert_eq!(Staking::current_era(), era_index);
}

pub fn current_total_payout_for_duration(duration: u64) -> u64 {
    inflation::compute_total_payout(
        <Test as Trait>::RewardCurve::get(),
        <Module<Test>>::slot_stake() * 2,
        Balances::total_issuance(),
        duration,
    )
    .0
}

pub fn reward_all_elected() {
    let rewards = <Module<Test>>::current_elected()
        .iter()
        .map(|v| (*v, 1))
        .collect::<Vec<_>>();

    <Module<Test>>::reward_by_ids(rewards)
}

pub fn validator_controllers() -> Vec<AccountId> {
    Session::validators()
        .into_iter()
        .map(|s| Staking::bonded(&s).expect("no controller for validator"))
        .collect()
}

pub fn on_offence_in_era(
    offenders: &[OffenceDetails<
        AccountId,
        pallet_session::historical::IdentificationTuple<Test>,
    >],
    slash_fraction: &[Perbill],
    era: EraIndex,
) {
    let bonded_eras = crate::BondedEras::get();
    for &(bonded_era, start_session) in bonded_eras.iter() {
        if bonded_era == era {
            Staking::on_offence(offenders, slash_fraction, start_session);
            return;
        } else if bonded_era > era {
            break;
        }
    }

    if Staking::current_era() == era {
        Staking::on_offence(
            offenders,
            slash_fraction,
            Staking::current_era_start_session_index(),
        );
    } else {
        panic!("cannot slash in era {}", era);
    }
}

pub fn on_offence_now(
    offenders: &[OffenceDetails<
        AccountId,
        pallet_session::historical::IdentificationTuple<Test>,
    >],
    slash_fraction: &[Perbill],
) {
    let now = Staking::current_era();
    on_offence_in_era(offenders, slash_fraction, now)
}
